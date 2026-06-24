//! ASP-owned Org artifact archival commands.

use super::org_capture::org_artifacts_root_for_project;
use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

pub(crate) fn run_org_archive_command(args: &[String]) -> Result<(), String> {
    let args = OrgArchiveArgs::parse(args)?;
    if args.help {
        println!("{}", archive_usage());
        return Ok(());
    }
    match args.command {
        OrgArchiveCommand::Done => archive_done(args),
    }
}

struct OrgArchiveArgs {
    help: bool,
    command: OrgArchiveCommand,
    artifacts_root: Option<PathBuf>,
    archive_dir: String,
    dry_run: bool,
}

#[derive(Clone, Copy)]
enum OrgArchiveCommand {
    Done,
}

struct ArchiveAction {
    source: PathBuf,
    target: PathBuf,
    rel: PathBuf,
}

impl OrgArchiveArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self {
            help: false,
            command: OrgArchiveCommand::Done,
            artifacts_root: None,
            archive_dir: "archives".to_string(),
            dry_run: false,
        };
        let mut saw_command = false;
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "-h" | "--help" | "help" => parsed.help = true,
                "done" if !saw_command => {
                    parsed.command = OrgArchiveCommand::Done;
                    saw_command = true;
                }
                "--artifacts-root" => {
                    index += 1;
                    parsed.artifacts_root = Some(PathBuf::from(required_flag_value(
                        args,
                        index,
                        "--artifacts-root",
                    )?));
                }
                "--archive-dir" => {
                    index += 1;
                    parsed.archive_dir = required_flag_value(args, index, "--archive-dir")?.into();
                }
                "--dry-run" => parsed.dry_run = true,
                _ if arg.starts_with('-') => return Err(format!("unknown archive flag `{arg}`")),
                _ => return Err(format!("unknown archive subcommand `{arg}`")),
            }
            index += 1;
        }
        if !saw_command && !parsed.help {
            return Err(archive_usage().to_string());
        }
        validate_archive_dir(&parsed.archive_dir)?;
        Ok(parsed)
    }
}

fn archive_done(args: OrgArchiveArgs) -> Result<(), String> {
    let project_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let artifacts_root = match args.artifacts_root {
        Some(path) if path.is_absolute() => path,
        Some(path) => project_root.join(path),
        None => org_artifacts_root_for_project(&project_root)?,
    };
    let actions = archive_actions(
        collect_done_org_files(&artifacts_root, &args.archive_dir),
        &artifacts_root,
        &args.archive_dir,
    )?;

    println!("[ASP_ORG_ARCHIVE] done");
    println!(
        "artifacts-root: {}",
        display_path(&project_root, &artifacts_root)
    );
    println!("archive-dir: {}", args.archive_dir);
    println!("dry-run: {}", args.dry_run);

    for action in &actions {
        println!(
            "- {} -> {}",
            display_path(&project_root, &action.source),
            display_path(&project_root, &action.target)
        );
        if !args.dry_run {
            archive_org_file(&action.source, &action.target, &action.rel)?;
        }
    }
    println!("archived-count: {}", actions.len());
    if actions.is_empty() {
        println!("status: no-done-records");
    }
    Ok(())
}

fn archive_actions(
    done_files: Vec<PathBuf>,
    artifacts_root: &Path,
    archive_dir: &str,
) -> Result<Vec<ArchiveAction>, String> {
    done_files
        .into_iter()
        .map(|source| {
            let rel = source
                .strip_prefix(artifacts_root)
                .map_err(|error| {
                    format!(
                        "failed to compute relative archive path for {}: {error}",
                        source.display()
                    )
                })?
                .to_path_buf();
            let target = artifacts_root.join(archive_dir).join(&rel);
            Ok(ArchiveAction {
                source,
                target,
                rel,
            })
        })
        .collect()
}

fn collect_done_org_files(root: &Path, archive_dir: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_done_org_files_into(root, archive_dir, &mut files);
    files.sort();
    files
}

fn collect_done_org_files_into(root: &Path, archive_dir: &str, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if should_descend_archive_scan_dir(&name, archive_dir) {
                collect_done_org_files_into(&path, archive_dir, files);
            }
            continue;
        }
        if file_type.is_file() && is_org_file_path(&path) && org_file_has_done_heading(&path) {
            files.push(path);
        }
    }
}

fn should_descend_archive_scan_dir(name: &str, archive_dir: &str) -> bool {
    !matches!(name, ".git" | "archive" | "archives") && name != archive_dir
}

fn is_org_file_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("org")
}

fn org_file_has_done_heading(path: &Path) -> bool {
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    org_source_has_done_heading(&source)
}

fn org_source_has_done_heading(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix('*') else {
            return false;
        };
        rest.trim_start().starts_with("DONE ")
    })
}

fn archive_org_file(source: &Path, target: &Path, rel: &Path) -> Result<(), String> {
    if target.exists() {
        return Err(format!(
            "archive target already exists: {}",
            target.display()
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let source_text = fs::read_to_string(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
    fs::write(target, archived_org_source(&source_text, rel))
        .map_err(|error| format!("failed to write {}: {error}", target.display()))?;
    fs::remove_file(source)
        .map_err(|error| format!("failed to remove {}: {error}", source.display()))
}

fn archived_org_source(source: &str, rel: &Path) -> String {
    let archived_from = rel.to_string_lossy().replace('\\', "/");
    let mut archived = format!("#+ARCHIVED_FROM: {archived_from}\n#+ARCHIVE_REASON: done\n");
    archived.push_str(source);
    if !archived.ends_with('\n') {
        archived.push('\n');
    }
    archived
}

fn validate_archive_dir(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        return Err("archive dir must be a non-empty relative path".to_string());
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("archive dir must not contain path traversal or separators".to_string());
    }
    Ok(())
}

fn required_flag_value<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn archive_usage() -> &'static str {
    "usage: asp org archive done [--artifacts-root PATH] [--archive-dir DIR] [--dry-run]\n\n`archive done` moves .org files containing DONE task headings from the active ASP Org artifacts tree into DIR while preserving their relative path and adding ARCHIVED_FROM metadata. The default artifacts root is .cache/agent-semantic-protocol/artifacts/org for the current project, and the default archive dir is archives. Run with --dry-run after `asp org query --kind task --field todo=DONE --exclude-dir archives --workspace <artifacts-root> --content` to review the parser-selected DONE tasks before moving files."
}
