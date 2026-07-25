//! Process-global environment serialization for integration tests.

pub(crate) static ASP_STATE_HOME_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
