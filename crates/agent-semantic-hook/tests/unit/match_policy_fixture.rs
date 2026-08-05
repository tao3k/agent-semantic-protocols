//! Serializes executable capability projections used by matcher contract tests.

static MATCH_POLICY_CAPABILITY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn capability_guard() -> std::sync::MutexGuard<'static, ()> {
    MATCH_POLICY_CAPABILITY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
