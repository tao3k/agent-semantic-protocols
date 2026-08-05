//! Serializes process-heavy Git and installer fixtures without constraining normal tests.

pub(crate) static GIT_FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static INSTALL_FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn install_fixture_guard() -> std::sync::MutexGuard<'static, ()> {
    INSTALL_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn isolated_git_command() -> std::process::Command {
    let mut command = std::process::Command::new("git");
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}
