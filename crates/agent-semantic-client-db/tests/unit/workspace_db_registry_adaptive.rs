use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tempfile::TempDir;

use super::{
    Mutex, ProviderSearchWorkspaceSession, WorkspaceDbEntry, run_workspace_db_writer_actor,
    workspace_db_reader_connection_limit, workspace_db_writer_channel,
    workspace_db_writer_concurrency_plan,
};

#[tokio::test(flavor = "multi_thread")]
async fn reader_pool_grows_to_runtime_demand_and_reuses_connections() {
    let temp = TempDir::new().expect("create adaptive reader tempfile");
    let client_db_path = temp.path().join("facts.turso");
    let database = turso::Builder::new_local(
        client_db_path
            .to_str()
            .expect("adaptive reader database path is UTF-8"),
    )
    .build()
    .await
    .expect("open adaptive reader database");
    let read_connection = Arc::new(database.connect().expect("create initial read connection"));
    let writer_connection = database.connect().expect("create writer connection");
    let connection_create_count = Arc::new(AtomicU64::new(2));
    let (writer_queue_capacity, writer_batch_limit) = workspace_db_writer_concurrency_plan();
    let (writer_client, writer_actor) = workspace_db_writer_channel(writer_queue_capacity);
    let writer_task = tokio::spawn(run_workspace_db_writer_actor(
        writer_actor,
        writer_connection,
        writer_batch_limit,
    ));
    let reader_limit = workspace_db_reader_connection_limit();
    let session = ProviderSearchWorkspaceSession {
        entry: Arc::new(WorkspaceDbEntry {
            workspace_identity: "adaptive-readers".to_owned(),
            client_db_path: PathBuf::from(&client_db_path),
            _database: database,
            read_connections: Mutex::new(vec![read_connection]),
            active_reader_count: AtomicU64::new(0),
            max_reader_connection_count: reader_limit,
            next_read_connection: AtomicU64::new(0),
            connection_create_count: Arc::clone(&connection_create_count),
            source_index_read_cache: (0..reader_limit)
                .map(|_| tokio::sync::Mutex::new(None))
                .collect(),
            writer_client,
            writer_task,
            next_request_id: AtomicU64::new(0),
            writer_transaction_count: AtomicU64::new(0),
            max_active_writer_count: AtomicU64::new(0),
        }),
    };

    let reader_count = reader_limit.saturating_mul(2);
    let barrier = Arc::new(tokio::sync::Barrier::new(reader_count + 1));
    let mut readers = tokio::task::JoinSet::new();
    for _ in 0..reader_count {
        let session = session.clone();
        let barrier = Arc::clone(&barrier);
        readers.spawn(async move {
            let _read_lease = session.read_connection();
            barrier.wait().await;
        });
    }
    barrier.wait().await;
    while let Some(reader) = readers.join_next().await {
        reader.expect("adaptive reader task must join");
    }

    assert_eq!(
        connection_create_count.load(Ordering::Relaxed),
        reader_limit as u64 + 1
    );
    {
        let _read_lease = session.read_connection();
    }
    assert_eq!(
        connection_create_count.load(Ordering::Relaxed),
        reader_limit as u64 + 1
    );
}
