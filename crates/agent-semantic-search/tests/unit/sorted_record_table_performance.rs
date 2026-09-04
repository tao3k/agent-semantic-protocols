//! Warm mmap table performance gates owned by Search.

use std::time::Duration;
use std::time::Instant;

use super::ValidatedSortedRecordTable;
use super::encode_sorted_record_table;

#[test]
fn large_directory_table_parse_p99_is_sub_millisecond() {
    const RECORDS: usize = 131_072;
    const RUNS: usize = 1_000;
    let records = (0..RECORDS)
        .map(|index| {
            (
                format!("symbol-{index:08x}").into_bytes(),
                format!("owner-{index:08x}").into_bytes(),
            )
        })
        .collect();
    let encoded = encode_sorted_record_table(records).expect("encode large search directory");
    let mut samples = Vec::with_capacity(RUNS);

    for _ in 0..RUNS {
        let started = Instant::now();
        let table = ValidatedSortedRecordTable::parse(&encoded).expect("open search directory");
        assert_eq!(table.len(), RECORDS);
        samples.push(started.elapsed());
    }

    samples.sort_unstable();
    let p99_index = samples.len().saturating_mul(99).div_ceil(100) - 1;
    let p99 = samples[p99_index];
    assert!(
        p99 < Duration::from_millis(1),
        "large search directory open p99 must remain sub-millisecond: p99={p99:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_memory_search_readers_are_lock_free() {
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
