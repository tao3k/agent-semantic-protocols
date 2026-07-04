//! Shared Turso statement execution helpers.

use std::future::Future;

use super::turso_lock_policy::{
    TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS, is_turso_lock_error, turso_lock_retry_delay,
};

macro_rules! execute_turso_prepared_statement_with_lock_retry {
    ($statement:expr, $params:expr, $context:expr $(,)?) => {{
        let context = $context;
        let mut attempt = 0usize;
        loop {
            match $statement.execute($params).await {
                Ok(row_count) => break Ok(row_count),
                Err(error) => {
                    let message = format!("{context}: {error}");
                    if !$crate::engine::turso_lock_policy::is_turso_lock_error(&message) {
                        break Err(message);
                    }
                    attempt += 1;
                    if attempt
                        >= $crate::engine::turso_lock_policy::TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS
                    {
                        break Err(format!(
                            "{} after {} retry attempts",
                            message,
                            $crate::engine::turso_lock_policy::TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS
                        ));
                    }
                    tokio::time::sleep($crate::engine::turso_lock_policy::turso_lock_retry_delay(
                        attempt - 1,
                    ))
                    .await;
                }
            }
        }
    }};
}

pub(crate) use execute_turso_prepared_statement_with_lock_retry;

/// Run one Turso operation with the DB Engine lock retry policy.
pub(crate) async fn run_turso_operation_with_lock_retry<T, F, Fut>(
    mut operation: F,
    context: &str,
) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let mut last_lock_error = None;
    for attempt in 0..TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS {
        match operation().await {
            Ok(count) => return Ok(count),
            Err(message) => {
                let message = format!("{context}: {message}");
                if !is_turso_lock_error(&message) {
                    return Err(message);
                }
                last_lock_error = Some(message);
            }
        }
        tokio::time::sleep(turso_lock_retry_delay(attempt)).await;
    }
    Err(format!(
        "{} after {} retry attempts",
        last_lock_error.unwrap_or_else(|| context.to_string()),
        TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS
    ))
}

/// Execute one Turso statement with the DB Engine lock retry policy.
pub(crate) async fn execute_turso_operation_with_lock_retry<F, Fut>(
    operation: F,
    context: &str,
) -> Result<u64, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<u64, String>>,
{
    run_turso_operation_with_lock_retry(operation, context).await
}

/// Execute a schema/control statement with the DB Engine Turso lock policy.
pub(crate) async fn execute_turso_statement_with_lock_retry(
    connection: &turso::Connection,
    statement: &str,
    context: &str,
) -> Result<(), String> {
    run_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(statement, ())
                .await
                .map_err(|error| error.to_string())
        },
        context,
    )
    .await
    .map(|_| ())
}
