//! Project state synchronization for `asp sync`.

use super::org_capture::{org_artifacts_root_for_project, run_org_state_sync};
use std::env;
use std::path::{Path, PathBuf};

pub(crate) fn run_sync_command(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        println!("{}", usage());
        return Ok(());
    }
    let project_root = project_root_arg(args)?;
    let sync = run_org_state_sync(&project_root)?;
    let org_state = agent_semantic_runtime::project_state_paths(&project_root)?
        .protocol_home
        .join("org");
    let org_artifacts = org_artifacts_root_for_project(&project_root)?;
    println!(
        "[asp-sync] orgState={} orgArtifacts={} orgRepo={} orgStatus={} copiedFiles={}",
        display_path(&project_root, &org_state),
        display_path(&project_root, &org_artifacts),
        sync.source,
        sync.status,
        sync.copied_files,
    );
    if let Some(backup) = sync.legacy_backup.as_ref() {
        println!("|legacyBackup={}", display_path(&project_root, backup));
    }
    Ok(())
}

fn project_root_arg(args: &[String]) -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|error| format!("failed to read current dir: {error}"))?;
    let root = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(if root.is_absolute() {
        root
    } else {
        cwd.join(root)
    })
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn usage() -> &'static str {
    "usage: asp sync [PROJECT_ROOT]\n\nSynchronizes project-owned ASP state. The Org resource tree is cloned or fast-forwarded from ASP_ORG_REPO_URL, defaulting to https://github.com/tao3k/org.git. Agent-authored Org state belongs under artifacts/org."
}
