//! Lock and retry policy shared by local Turso DB engine owners.

use std::time::Duration;

pub(crate) const TURSO_CLIENT_DB_BUSY_TIMEOUT_MS: u64 = 5_000;
pub(crate) const TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS: usize = 80;
pub(crate) const TURSO_CLIENT_DB_LOCK_RETRY_BASE_MS: u64 = 5;
pub(crate) const TURSO_CLIENT_DB_LOCK_RETRY_MAX_MS: u64 = 200;
pub(crate) const TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS: usize = 80;

pub(crate) fn is_turso_lock_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("locking error")
        || message.contains("locked")
        || message.contains("wal coordination map magic mismatch")
        || message.contains("coordination file is smaller than the coordination header")
        || message.contains("coordination header")
        || message.contains("magic mismatch")
        || message.contains("busy")
}

pub(crate) fn turso_lock_retry_delay(attempt: usize) -> Duration {
    let multiplier = 1_u64 << attempt.min(5);
    Duration::from_millis(
        TURSO_CLIENT_DB_LOCK_RETRY_BASE_MS
            .saturating_mul(multiplier)
            .min(TURSO_CLIENT_DB_LOCK_RETRY_MAX_MS),
    )
}
