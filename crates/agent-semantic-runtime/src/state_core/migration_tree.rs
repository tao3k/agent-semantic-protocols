//! Immutable-tree and Hook-event merge algorithms for State Core migration.

use std::fs;
use std::path::Path;

use sha2::Digest;
use sha2::Sha256;

use super::migration::commit_text_atomically;
use super::migration::io_error;
use super::migration::remove_file_if_present;
use super::migration::sha256_hex;

pub(super) fn resolve_retired_tree_target(
    source: &Path,
    preferred: &Path,
) -> Result<std::path::PathBuf, String> {
    if !preferred.exists() || immutable_trees_are_merge_compatible(source, preferred)? {
        return Ok(preferred.to_path_buf());
    }
    let digest = immutable_tree_digest(source)?;
    let file_name = preferred
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "retired state target has no UTF-8 file name: {}",
                preferred.display()
            )
        })?;
    let generation = preferred.with_file_name(format!("{file_name}--sha256-{digest}"));
    if generation.exists() && !immutable_trees_are_merge_compatible(source, &generation)? {
        return Err(format!(
            "state migration conflict: digest-addressed retired generation differs: source={} target={}",
            source.display(),
            generation.display()
        ));
    }
    Ok(generation)
}

fn immutable_trees_are_merge_compatible(source: &Path, target: &Path) -> Result<bool, String> {
    if !source.exists() {
        return Ok(true);
    }
    for entry in fs::read_dir(source).map_err(io_error("read immutable migration source"))? {
        let entry = entry.map_err(io_error("read immutable migration entry"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let source_type = entry
            .file_type()
            .map_err(io_error("read immutable migration file type"))?;
        if !target_path.exists() {
            if !source_type.is_dir() && !source_type.is_file() {
                return Err(format!(
                    "state migration conflict: immutable tree contains non-file entry: {}",
                    source_path.display()
                ));
            }
            continue;
        }
        let target_type = fs::symlink_metadata(&target_path)
            .map_err(io_error("read immutable migration target type"))?
            .file_type();
        if source_type.is_dir() && target_type.is_dir() {
            if !immutable_trees_are_merge_compatible(&source_path, &target_path)? {
                return Ok(false);
            }
            continue;
        }
        if source_type.is_file() && target_type.is_file() {
            if !files_are_identical(&source_path, &target_path)? {
                return Ok(false);
            }
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

fn immutable_tree_digest(root: &Path) -> Result<String, String> {
    use std::io::Read;

    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(io_error("read retired generation digest directory"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error("read retired generation digest entry"))?;
        for entry in entries {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(io_error("read retired generation digest type"))?;
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() {
                files.push(path);
            } else {
                return Err(format!(
                    "state migration conflict: retired generation contains non-file entry: {}",
                    path.display()
                ));
            }
        }
    }
    files.sort();

    let mut hasher = Sha256::new();
    for path in files {
        let relative = path.strip_prefix(root).map_err(|error| {
            format!(
                "retired generation digest path escaped root: root={} path={} error={error}",
                root.display(),
                path.display()
            )
        })?;
        let relative = relative.to_string_lossy();
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        let mut file =
            fs::File::open(&path).map_err(io_error("open retired generation digest file"))?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(io_error("read retired generation digest file"))?;
            hasher.update((read as u64).to_le_bytes());
            hasher.update(&buffer[..read]);
            if read == 0 {
                break;
            }
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn merge_immutable_tree(source: &Path, target: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(source).map_err(io_error("read immutable migration source"))? {
        let entry = entry.map_err(io_error("read immutable migration entry"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(io_error("read immutable migration file type"))?;
        if file_type.is_dir() {
            merge_immutable_tree(&source_path, &target_path)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(format!(
                "state migration conflict: immutable tree contains non-file entry: {}",
                source_path.display()
            ));
        }
        if target_path.exists() {
            if !files_are_identical(&source_path, &target_path)? {
                return Err(format!(
                    "state migration conflict: immutable artifact differs: source={} target={}",
                    source_path.display(),
                    target_path.display()
                ));
            }
            fs::remove_file(&source_path)
                .map_err(io_error("remove duplicate immutable artifact"))?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(io_error("create immutable migration target"))?;
            }
            fs::rename(&source_path, &target_path)
                .map_err(io_error("migrate immutable artifact"))?;
        }
    }
    fs::remove_dir(source).map_err(io_error("remove immutable migration source dir"))
}

fn files_are_identical(left: &Path, right: &Path) -> Result<bool, String> {
    use std::io::Read;

    let left_metadata = fs::metadata(left).map_err(io_error("read left artifact metadata"))?;
    let right_metadata = fs::metadata(right).map_err(io_error("read right artifact metadata"))?;
    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }
    let mut left_file = fs::File::open(left).map_err(io_error("open left immutable artifact"))?;
    let mut right_file =
        fs::File::open(right).map_err(io_error("open right immutable artifact"))?;
    let mut left_buffer = [0_u8; 64 * 1024];
    let mut right_buffer = [0_u8; 64 * 1024];
    loop {
        let left_read = left_file
            .read(&mut left_buffer)
            .map_err(io_error("read left immutable artifact"))?;
        let right_read = right_file
            .read(&mut right_buffer)
            .map_err(io_error("read right immutable artifact"))?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

pub(super) fn merge_hook_event_logs(
    legacy: &Path,
    canonical: &Path,
) -> Result<(usize, String), String> {
    use std::collections::BTreeMap;

    if !legacy.exists() {
        return Ok((0, sha256_hex(b"")));
    }
    let legacy_content = fs::read(legacy).map_err(io_error("read legacy hook event window"))?;
    let window_digest = sha256_hex(&legacy_content);
    let mut events = BTreeMap::new();
    if canonical.exists() {
        collect_hook_events(canonical, &mut events)?;
    }
    let canonical_count = events.len();
    collect_hook_events(legacy, &mut events)?;
    let merged_event_count = events.len().saturating_sub(canonical_count);
    let mut content = String::new();
    for event in events.values() {
        content.push_str(
            &serde_json::to_string(event)
                .map_err(|error| format!("serialize merged hook event: {error}"))?,
        );
        content.push('\n');
    }
    write_text_atomically(canonical, &content)?;
    remove_file_if_present(legacy)?;
    Ok((merged_event_count, window_digest))
}

fn collect_hook_events(
    path: &Path,
    events: &mut std::collections::BTreeMap<(u64, String, String), serde_json::Value>,
) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(io_error("read hook event log"))?;
    for (index, line) in content.lines().enumerate() {
        let event: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            format!(
                "parse hook event log {} line {}: {error}",
                path.display(),
                index + 1
            )
        })?;
        let recorded_at = event
            .get("recordedAtUnixMs")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "hook event missing recordedAtUnixMs: {} line {}",
                    path.display(),
                    index + 1
                )
            })?;
        let event_kind = event
            .get("event")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "hook event missing event kind: {} line {}",
                    path.display(),
                    index + 1
                )
            })?
            .to_string();
        let event_identity = if let Some(tool_use_id) = event
            .pointer("/fields/toolUseId")
            .and_then(serde_json::Value::as_str)
        {
            tool_use_id.to_string()
        } else {
            let encoded = serde_json::to_vec(&event)
                .map_err(|error| format!("serialize hook event identity: {error}"))?;
            let mut hasher = Sha256::new();
            hasher.update(encoded);
            format!("event-json:{:x}", hasher.finalize())
        };
        let key = (recorded_at, event_identity, event_kind);
        if let Some(existing) = events.get(&key) {
            if existing != &event {
                return Err(format!(
                    "hook event identity conflict: {} line {}",
                    path.display(),
                    index + 1
                ));
            }
        } else {
            events.insert(key, event);
        }
    }
    Ok(())
}

fn write_text_atomically(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("state text path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(io_error("create atomic state text parent"))?;
    let temp_path = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("state text path has no file name: {}", path.display()))?
    ));
    commit_text_atomically(&temp_path, path, content)
}
