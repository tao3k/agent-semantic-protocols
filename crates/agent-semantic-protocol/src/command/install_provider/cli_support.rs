use std::path::{Path, PathBuf};

/// Return whether the install invocation requested help.
pub(super) fn has_help_flag(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
}

pub(super) fn absolute_project_root(invocation_root: &Path, project_root: &Path) -> PathBuf {
    if project_root.is_absolute() {
        project_root.to_path_buf()
    } else {
        invocation_root.join(project_root)
    }
}

pub(super) fn usage() -> String {
    "usage: asp install binary\n       asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin --codex [PROJECT_ROOT]\n       asp install language <language> [--global | --project <canonical-root>] [--target <target>]\n       plugin scope: global only; omitted PROJECT_ROOT resolves ASP_STATE_HOME [dev].root\n       language scope: global is the default; project installation requires explicit --project <canonical-root>\n       release mode: plain `asp install language` resolves only the locked release artifact (installMode=locked-release)\n       develop mode: plain `asp install language` delegates to the development installer under [dev].root; [dev].root owns provider builds and installation (installMode=develop-workspace)".to_string()
}

pub(super) fn install_hook_usage() -> String {
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]".to_string()
}
