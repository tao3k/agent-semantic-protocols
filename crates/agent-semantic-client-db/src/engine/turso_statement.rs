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

/// Execute one mutating Turso statement and report whether it changed storage.
///
/// `Connection::execute` and this Turso runtime's SQL `changes()` projection expose
/// connection-cumulative physical counts, including index work. Compute a delta from
/// `total_changes()` only as a mutation signal; callers that need a logical row count
/// must query the owning table explicitly.
pub(crate) async fn execute_turso_operation_with_statement_change_signal<F, Fut>(
    connection: &turso::Connection,
    mut operation: F,
    context: &str,
) -> Result<bool, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<u64, String>>,
{
    run_turso_operation_with_lock_retry(
        || {
            let operation = operation();
            async move {
                let changes_before = read_turso_total_changes(connection).await?;
                let _cumulative_change_count = operation.await?;
                let changes_after = read_turso_total_changes(connection).await?;
                changes_after
                    .checked_sub(changes_before)
                    .ok_or_else(|| {
                        format!(
                            "Turso total changes regressed around one statement: before={changes_before} after={changes_after}"
                        )
                    })
                    .map(|changed| u64::from(changed > 0))
            }
        },
        context,
    )
    .await
    .map(|changed| changed > 0)
}

async fn read_turso_total_changes(connection: &turso::Connection) -> Result<u64, String> {
    let mut rows = connection
        .query("SELECT total_changes()", ())
        .await
        .map_err(|error| error.to_string())?;
    let row = rows
        .next()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "SELECT total_changes() returned no row".to_string())?;
    let changes = row.get::<i64>(0).map_err(|error| error.to_string())?;
    u64::try_from(changes)
        .map_err(|_| format!("SELECT total_changes() returned a negative count: {changes}"))
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
