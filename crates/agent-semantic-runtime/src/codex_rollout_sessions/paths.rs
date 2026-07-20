//! Codex rollout JSONL session index parser.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};



pub(crate) fn codex_rollout_paths_for_session_id(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let search_roots = rollout_search_roots_for_session_id(sessions_dir, session_id);
    let mut paths = direct_rollout_paths_for_session_id(&search_roots, session_id)?;
    if paths.is_empty() {
        for search_root in &search_roots {
            if search_root.is_dir() {
                paths.extend(rg_rollout_paths_for_session_id(search_root, session_id)?);
            }
        }
    }
    if paths.is_empty() {
        for search_root in &search_roots {
            if search_root.is_dir() {
                paths.extend(recursive_rollout_paths_for_session_id(
                    search_root,
                    session_id,
                    3,
                )?);
            }
        }
    }
    if paths.is_empty() {
        return Err(format!(
            "Codex rollout invariant broken: no rollout JSONL found for session {session_id} under {}",
            sessions_dir.display()
        ));
    }
    paths.retain(|path| path.is_file());
    paths.sort();
    paths.dedup();
    paths.reverse();
    Ok(paths)
}

fn direct_rollout_paths_for_session_id(
    search_roots: &[PathBuf],
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    if let Some(file_name) = rollout_exact_filename_for_session_id(session_id) {
        let paths = search_roots
            .iter()
            .map(|search_root| search_root.join(&file_name))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        if !paths.is_empty() {
            return Ok(paths);
        }
    }
    let suffix = format!("{session_id}.jsonl");
    let mut paths = Vec::new();
    for search_root in search_roots {
        if !search_root.is_dir() {
            continue;
        }
        for entry in fs::read_dir(search_root)
            .map_err(|error| format!("failed to read {}: {error}", search_root.display()))?
        {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read Codex session entry below {}: {error}",
                    search_root.display()
                )
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "failed to inspect Codex session entry {}: {error}",
                    path.display()
                )
            })?;
            if !file_type.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if file_name.starts_with("rollout-") && file_name.ends_with(&suffix) {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

fn rollout_exact_filename_for_session_id(session_id: &str) -> Option<String> {
    let unix_millis = uuid_v7_unix_millis(session_id)?;
    let unix_seconds = unix_millis.div_euclid(1_000);
    let unix_day = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_unix_day(unix_day);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    Some(format!(
        "rollout-{year:04}-{month:02}-{day:02}T{hour:02}-{minute:02}-{second:02}-{session_id}.jsonl"
    ))
}

fn rollout_search_roots_for_session_id(sessions_dir: &Path, session_id: &str) -> Vec<PathBuf> {
    let Some(unix_day) = uuid_v7_unix_day(session_id) else {
        return vec![sessions_dir.to_path_buf()];
    };
    (-1..=1)
        .map(|offset| rollout_date_dir(sessions_dir, unix_day + offset))
        .collect()
}

fn uuid_v7_unix_day(session_id: &str) -> Option<i64> {
    Some(uuid_v7_unix_millis(session_id)?.div_euclid(86_400_000))
}

fn uuid_v7_unix_millis(session_id: &str) -> Option<i64> {
    let uuid = uuid::Uuid::parse_str(session_id).ok()?;
    let timestamp = uuid.get_timestamp()?;
    let (seconds, nanos) = timestamp.to_unix();
    let millis = seconds
        .checked_mul(1_000)?
        .checked_add(u64::from(nanos.checked_div(1_000_000).unwrap_or_default()))?;
    i64::try_from(millis).ok()
}

fn rollout_date_dir(sessions_dir: &Path, unix_day: i64) -> PathBuf {
    let (year, month, day) = civil_from_unix_day(unix_day);
    sessions_dir
        .join(format!("{year:04}"))
        .join(format!("{month:02}"))
        .join(format!("{day:02}"))
}

fn civil_from_unix_day(unix_day: i64) -> (i32, u32, u32) {
    let days = unix_day + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 }.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524
        - day_of_era / 146_096)
        .div_euclid(365);
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2).div_euclid(153);
    let day = day_of_year - (153 * month_prime + 2).div_euclid(5) + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year as i32, month as u32, day as u32)
}

fn recursive_rollout_paths_for_session_id(
    search_root: &Path,
    session_id: &str,
    max_depth: usize,
) -> Result<Vec<PathBuf>, String> {
    let suffix = format!("{session_id}.jsonl");
    let mut paths = Vec::new();
    collect_rollout_paths_bounded(search_root, &suffix, max_depth, 0, &mut paths)?;
    Ok(paths)
}

fn collect_rollout_paths_bounded(
    root: &Path,
    suffix: &str,
    max_depth: usize,
    depth: usize,
    paths: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in
        fs::read_dir(root).map_err(|error| format!("failed to read {}: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read Codex session entry below {}: {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect Codex session entry {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_file() {
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if file_name.starts_with("rollout-") && file_name.ends_with(suffix) {
                paths.push(path);
            }
            continue;
        }
        if file_type.is_dir() && depth < max_depth {
            collect_rollout_paths_bounded(&path, suffix, max_depth, depth + 1, paths)?;
        }
    }
    Ok(())
}

pub(super) fn rg_rollout_paths_for_session_id(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let glob = format!("**/rollout-*{session_id}.jsonl");
    let output = match Command::new("rg")
        .arg("--files")
        .arg("--glob")
        .arg(glob)
        .arg(sessions_dir)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to run rg for Codex sessions dir {}: {error}",
                sessions_dir.display()
            ));
        }
    };
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(format!(
            "rg failed while locating Codex rollout for session {session_id}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let path = PathBuf::from(line);
            if path.is_absolute() {
                path
            } else {
                sessions_dir.join(path)
            }
        })
        .collect())
}
pub(super) fn codex_sessions_dir() -> Result<PathBuf, String> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(codex_home).join("sessions"));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".codex").join("sessions"))
        .ok_or_else(|| "HOME is not set; cannot locate Codex sessions".to_string())
}
