use std::time::{Duration, Instant};

use super::{ValidatedSortedRecordTable, encode_sorted_record_table};

#[test]
fn thirty_run_large_directory_cold_open_p99_is_sub_millisecond() {
    const RECORDS: usize = 131_072;
    let records = (0..RECORDS)
        .map(|index| {
            (
                format!("symbol-{index:08x}").into_bytes(),
                format!("owner-{index:08x}").into_bytes(),
            )
        })
        .collect();
    let encoded = encode_sorted_record_table(records).expect("encode large search directory");
    let mut samples = Vec::with_capacity(30);

    for _ in 0..30 {
        let started = Instant::now();
        let table = ValidatedSortedRecordTable::parse(&encoded).expect("open search directory");
        assert_eq!(table.len(), RECORDS);
        samples.push(started.elapsed());
    }

    samples.sort_unstable();
    let p99 = samples[samples.len() - 1];
    eprintln!(
        "search-memory-table-cold-open records={RECORDS} runs=30 p99Nanos={}",
        p99.as_nanos()
    );
    assert!(
        p99 < Duration::from_millis(1),
        "large search directory cold-open p99 must remain sub-millisecond: p99={p99:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn two_hundred_fifty_six_concurrent_memory_search_readers_are_lock_free() {
    use std::sync::Arc;

    let records = (0..4_096usize)
        .map(|index| {
            (
                format!("symbol-{index:08x}").into_bytes(),
                format!("owner-{index:08x}").into_bytes(),
            )
        })
        .collect();
    let encoded =
        Arc::new(encode_sorted_record_table(records).expect("encode concurrent search directory"));
    let barrier = Arc::new(tokio::sync::Barrier::new(257));
    let mut readers = tokio::task::JoinSet::new();

    for _ in 0..256 {
        let encoded = Arc::clone(&encoded);
        let barrier = Arc::clone(&barrier);
        readers.spawn(async move {
            barrier.wait().await;
            let table = ValidatedSortedRecordTable::parse(encoded.as_slice())
                .expect("open immutable table");
            assert_eq!(
                table
                    .get_checked(b"symbol-00000fff")
                    .expect("validated lookup")
                    .expect("registered key"),
                b"owner-00000fff"
            );
        });
    }

    barrier.wait().await;
    while let Some(result) = readers.join_next().await {
        result.expect("memory search reader task completes");
    }
}

#[test]
fn memory_search_cost_receipt_is_opentelemetry_serializable() {
    let observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "exact-query",
        "resident-exact-generation-open",
        50,
        1_000,
        "within-budget",
    )
    .with_memory_search_metrics(52_000_000, 512, 128, 256, 64);
    let wire = serde_json::to_value(observation).expect("serialize memory search metrics");

    assert_eq!(wire["memorySearchMappedBytes"], 52_000_000);
    assert_eq!(wire["memorySearchDirectoryBytesValidated"], 512);
    assert_eq!(wire["memorySearchKeyBytesTouched"], 128);
    assert_eq!(wire["memorySearchValueBytesTouched"], 256);
    assert_eq!(wire["memorySearchSourceBytesRead"], 64);
    assert_eq!(wire["memorySearchTursoOpens"], 0);
    assert_eq!(wire["memorySearchSocketConnects"], 0);
    assert_eq!(wire["memorySearchProviderSpawns"], 0);
}
