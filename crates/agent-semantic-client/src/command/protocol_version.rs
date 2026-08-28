//! Shared CLI version rendering.

pub(in crate::command) fn protocol_version_line() -> String {
    format!("asp {}", env!("CARGO_PKG_VERSION"))
}

pub(in crate::command) fn run_protocol_version_command(args: &[String]) -> Result<(), String> {
    match args {
        [] => {
            println!("{}", protocol_version_line());
            Ok(())
        }
        [arg] if arg == "--profile" => {
            println!("{}", protocol_build_profile());
            Ok(())
        }
        [arg] if arg == "--require-release" => require_release_protocol_build(),
        _ => Err("usage: asp --version [--profile|--require-release]".to_string()),
    }
}

fn require_release_protocol_build() -> Result<(), String> {
    let profile = protocol_build_profile();
    if profile == "release" {
        println!("{} profile={profile}", protocol_version_line());
        return Ok(());
    }
    Err(format!(
        "[asp-build-profile-error] expected=release actual={profile}\n\
         |hint global installs and performance receipts require a release ASP artifact\n\
         nextCommand=just agent-tools-install-protocol"
    ))
}

fn protocol_build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
