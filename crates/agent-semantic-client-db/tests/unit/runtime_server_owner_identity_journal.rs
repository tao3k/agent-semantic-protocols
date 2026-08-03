use std::time::Duration;

use super::{
    RuntimeOwnerIdentityEntry, RuntimeOwnerIdentityJournalPublisher,
    RuntimeOwnerIdentityJournalReader, RuntimeOwnerIdentityState,
};

const BASE_A: &str = "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE_B: &str = "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OWNER_A: &str = "blake3-256:1111111111111111111111111111111111111111111111111111111111111111";
const OWNER_B: &str = "blake3-256:2222222222222222222222222222222222222222222222222222222222222222";

fn present(owner_path: &str, digest: &str) -> RuntimeOwnerIdentityEntry {
    RuntimeOwnerIdentityEntry {
        owner_path: owner_path.to_owned(),
        state: RuntimeOwnerIdentityState::Present,
        content_digest: Some(digest.to_owned()),
        mutation_id: None,
    }
}

fn missing(owner_path: &str) -> RuntimeOwnerIdentityEntry {
    RuntimeOwnerIdentityEntry {
        owner_path: owner_path.to_owned(),
        state: RuntimeOwnerIdentityState::Missing,
        content_digest: None,
        mutation_id: None,
    }
}

fn mutating(owner_path: &str, mutation_id: &str) -> RuntimeOwnerIdentityEntry {
    RuntimeOwnerIdentityEntry {
        owner_path: owner_path.to_owned(),
        state: RuntimeOwnerIdentityState::Mutating,
        content_digest: None,
        mutation_id: Some(mutation_id.to_owned()),
    }
}

async fn current(
    reader: &RuntimeOwnerIdentityJournalReader,
    base: &str,
    owner_path: &str,
    digest: &str,
) -> bool {
    reader
        .owner_is_current("workspace-a", base, owner_path, digest)
        .await
        .expect("read owner identity journal")
}

#[tokio::test]
async fn mutation_replay_is_idempotent_and_payload_conflict_is_rejected() {
    let temporary = tempfile::tempdir().expect("create owner journal fixture");
    let publisher = RuntimeOwnerIdentityJournalPublisher::open(temporary.path().to_path_buf())
        .await
        .expect("open owner journal publisher");
    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-1",
            vec![present("src/a.rs", OWNER_A), present("src/b.rs", OWNER_B)],
        )
        .await
        .expect("publish mutation");
    let committed_epoch = publisher
        .state
        .lock()
        .await
        .as_ref()
        .expect("committed snapshot")
        .epoch;
    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-1",
            vec![present("src/a.rs", OWNER_A), present("src/b.rs", OWNER_B)],
        )
        .await
        .expect("replay identical mutation");
    assert_eq!(
        publisher
            .state
            .lock()
            .await
            .as_ref()
            .expect("replayed snapshot")
            .epoch,
        committed_epoch
    );
    assert!(
        publisher
            .publish_delta(
                "workspace-a",
                BASE_A,
                "mutation-1",
                vec![present("src/a.rs", OWNER_B)],
            )
            .await
            .is_err()
    );
    assert!(
        publisher
            .publish_delta(
                "workspace-a",
                BASE_A,
                "mutation-1",
                vec![present("src/a.rs", OWNER_A)],
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn same_generation_rebase_preserves_delta_and_tombstone() {
    let temporary = tempfile::tempdir().expect("create owner journal fixture");
    let pointer = temporary.path().join("generation.pointer");
    let publisher = RuntimeOwnerIdentityJournalPublisher::open(temporary.path().to_path_buf())
        .await
        .expect("open owner journal publisher");
    publisher
        .rebase("workspace-a", BASE_A)
        .await
        .expect("publish base generation");
    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-1",
            vec![
                present("src/a.rs", OWNER_A),
                missing("src/deleted.rs"),
                mutating("src/editing.rs", "mutation-2"),
            ],
        )
        .await
        .expect("publish owner delta");
    publisher
        .rebase("workspace-a", BASE_A)
        .await
        .expect("same generation rebase");
    let reader = RuntimeOwnerIdentityJournalReader::open(&pointer)
        .await
        .expect("open owner journal reader");

    assert!(current(&reader, BASE_A, "src/a.rs", OWNER_A).await);
    assert!(!current(&reader, BASE_A, "src/a.rs", OWNER_B).await);
    assert!(!current(&reader, BASE_A, "src/deleted.rs", OWNER_A).await);
    assert!(!current(&reader, BASE_A, "src/editing.rs", OWNER_A).await);
    assert!(current(&reader, BASE_A, "src/from-base.rs", OWNER_A).await);
}

#[tokio::test]
async fn resident_reader_observes_delta_and_generation_rebase_without_reopen() {
    let temporary = tempfile::tempdir().expect("create owner journal fixture");
    let pointer = temporary.path().join("generation.pointer");
    let publisher = RuntimeOwnerIdentityJournalPublisher::open(temporary.path().to_path_buf())
        .await
        .expect("open owner journal publisher");
    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-1",
            vec![present("src/a.rs", OWNER_A)],
        )
        .await
        .expect("publish first delta");
    let resident_reader = RuntimeOwnerIdentityJournalReader::open(&pointer)
        .await
        .expect("open resident reader");
    let initial_counters = resident_reader.counter_snapshot();

    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-2",
            vec![present("src/a.rs", OWNER_B)],
        )
        .await
        .expect("publish second delta");
    assert!(!current(&resident_reader, BASE_A, "src/a.rs", OWNER_A).await);
    assert!(current(&resident_reader, BASE_A, "src/a.rs", OWNER_B).await);
    assert_eq!(
        resident_reader.counter_snapshot(),
        (
            initial_counters.0 + 1,
            initial_counters.1 + 1,
            initial_counters.2 + 1,
        )
    );

    publisher
        .rebase("workspace-a", BASE_B)
        .await
        .expect("publish next generation");
    assert!(!current(&resident_reader, BASE_A, "src/a.rs", OWNER_B).await);
    assert!(current(&resident_reader, BASE_B, "src/a.rs", OWNER_A).await);
    assert_eq!(
        resident_reader.counter_snapshot(),
        (
            initial_counters.0 + 2,
            initial_counters.1 + 2,
            initial_counters.2 + 2,
        )
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_atomic_publication_has_no_partial_reader_state() {
    let temporary = tempfile::tempdir().expect("create owner journal fixture");
    let pointer = temporary.path().join("generation.pointer");
    let publisher = RuntimeOwnerIdentityJournalPublisher::open(temporary.path().to_path_buf())
        .await
        .expect("open owner journal publisher");
    publisher
        .rebase("workspace-a", BASE_A)
        .await
        .expect("publish base generation");

    let mut readers = Vec::new();
    for _ in 0..16 {
        let pointer = pointer.clone();
        readers.push(tokio::spawn(async move {
            let reader = RuntimeOwnerIdentityJournalReader::open(&pointer)
                .await
                .expect("open concurrent resident reader");
            for _ in 0..256 {
                reader
                    .owner_is_current("workspace-a", BASE_A, "src/a.rs", OWNER_A)
                    .await
                    .expect("reader observes complete journal");
                tokio::task::yield_now().await;
            }
        }));
    }
    for mutation in 0..64 {
        let digest = if mutation % 2 == 0 { OWNER_A } else { OWNER_B };
        publisher
            .publish_delta(
                "workspace-a",
                BASE_A,
                &format!("mutation-{mutation}"),
                vec![present("src/a.rs", digest)],
            )
            .await
            .expect("publish atomic delta");
    }
    for reader in readers {
        reader.await.expect("join reader");
    }
}

#[tokio::test]
async fn load_once_journal_lookup_p99_is_sub_millisecond() {
    const SAMPLES: usize = 65_536;
    let temporary = tempfile::tempdir().expect("create owner journal fixture");
    let pointer = temporary.path().join("generation.pointer");
    let publisher = RuntimeOwnerIdentityJournalPublisher::open(temporary.path().to_path_buf())
        .await
        .expect("open owner journal publisher");
    publisher
        .publish_delta(
            "workspace-a",
            BASE_A,
            "mutation-1",
            (0..4_096)
                .map(|index| present(&format!("src/{index:04}.rs"), OWNER_A))
                .collect(),
        )
        .await
        .expect("publish lookup fixture");
    let reader = RuntimeOwnerIdentityJournalReader::open(&pointer)
        .await
        .expect("load journal once");
    let baseline_counters = reader.counter_snapshot();

    let mut latencies = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = tokio::time::Instant::now();
        assert!(current(&reader, BASE_A, "src/2048.rs", OWNER_A).await);
        latencies.push(started.elapsed());
    }
    latencies.sort_unstable();
    let p99 = latencies[SAMPLES * 99 / 100];
    let after_counters = reader.counter_snapshot();
    let segment_opens = after_counters.0 - baseline_counters.0;
    let segment_decodes = after_counters.1 - baseline_counters.1;
    let pointer_decodes = after_counters.2 - baseline_counters.2;
    eprintln!(
        "[runtime-owner-identity-journal-performance] samples={SAMPLES} entries=4096 p99Nanos={} budgetNanos=1000000 segmentOpensDelta={segment_opens} segmentDecodesDelta={segment_decodes} pointerDecodesDelta={pointer_decodes} dbOpens=0 providerInvocations=0 controlRoundtrips=0",
        p99.as_nanos(),
    );
    assert_eq!(segment_opens, 0, "warm lookup reopened a journal segment");
    assert_eq!(segment_decodes, 0, "warm lookup decoded a journal segment");
    assert_eq!(pointer_decodes, 0, "warm lookup decoded the stable pointer");
    assert!(
        p99 < Duration::from_millis(1),
        "load-once journal lookup p99 exceeded one millisecond: {p99:?}"
    );
}
