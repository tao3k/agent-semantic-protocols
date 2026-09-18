// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

/// Return whether the install invocation requested help.
pub(super) fn has_help_flag(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
}

pub(super) fn usage() -> String {
    "usage: asp install binary\n       asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin <status|publish> --codex [PROJECT_ROOT]\n       asp install language <language> [--target <target>]\n       plugin scope: global only; omitted PROJECT_ROOT resolves ASP_STATE_HOME [dev].root\n       language provider artifacts are published only through ASP State Home; Runtime workspace admission owns activation\n       release mode: plain `asp install language` resolves only the locked release artifact (installMode=locked-release)\n       develop mode: plain `asp install language` delegates to the development installer under [dev].root; [dev].root owns provider builds and installation (installMode=develop-workspace)".to_string()
}

pub(super) fn install_hook_usage() -> String {
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]".to_string()
}
