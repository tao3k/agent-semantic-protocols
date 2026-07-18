use crate::engine::turso_statement::execute_turso_statement_with_lock_retry;

pub(super) const TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(30);

pub(super) async fn rollback_turso_source_index_transaction<T>(
    connection: &turso::Connection,
    write_error: String,
) -> Result<T, String> {
    match execute_turso_statement_with_lock_retry(
        connection,
        "ROLLBACK",
        "failed to roll back Turso source-index transaction",
    )
    .await
    {
        Ok(()) => Err(write_error),
        Err(rollback_error) => Err(format!("{write_error}; rollbackError={rollback_error}")),
    }
}

pub(super) async fn return_turso_source_index_write_failure<T>(
    connection: &turso::Connection,
    transaction_started: bool,
    write_error: String,
) -> Result<T, String> {
    if transaction_started {
        rollback_turso_source_index_transaction(connection, write_error).await
    } else {
        Err(write_error)
    }
}

pub(super) struct TursoSourceIndexWriteStats {
    pub(super) physical_generation_id: String,
    pub(super) changed_owner_count: u32,
    pub(super) removed_owner_count: u32,
    pub(super) posting_write_count: u32,
}
