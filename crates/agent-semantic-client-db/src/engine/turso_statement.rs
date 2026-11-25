// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shared Turso statement execution helpers.

use std::future::Future;

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

/// Run one Turso operation and preserve its native failure boundary.
pub(crate) async fn run_turso_operation<T, F, Fut>(
    mut operation: F,
    context: &str,
) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    operation()
        .await
        .map_err(|message| format!("{context}: {message}"))
}

/// Execute one Turso operation without replaying partial transaction work.
pub(crate) async fn execute_turso_operation<F, Fut>(
    operation: F,
    context: &str,
) -> Result<u64, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<u64, String>>,
{
    run_turso_operation(operation, context).await
}

/// Execute a schema/control statement once and surface Turso's native error.
pub(crate) async fn execute_turso_statement(
    connection: &mrr_data_backend::turso_driver::Connection,
    statement: &str,
    context: &str,
) -> Result<(), String> {
    run_turso_operation(
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
