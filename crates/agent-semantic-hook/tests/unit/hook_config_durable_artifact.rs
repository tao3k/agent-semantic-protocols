use super::ClientHookConfig;

#[test]
fn complete_durable_hook_artifact_recovery_hydrate_p99_is_bounded() {
    let live = ClientHookConfig::default();
    let bytes = serde_json::to_vec(&live.durable_snapshot_config())
        .expect("serialize complete durable Hook matcher artifact");
    assert!(
        bytes.len() < 4 * 1024 * 1024,
        "complete durable Hook matcher artifact must fit the Runtime Server seqlock memory; observed {} bytes",
        bytes.len()
    );
    let mut samples = (0..100)
        .map(|_| {
            let started = std::time::Instant::now();
            let artifact = serde_json::from_slice(&bytes)
                .expect("decode complete durable Hook matcher artifact");
            let config = ClientHookConfig::from_durable_snapshot_config(artifact)
                .expect("hydrate complete durable Hook matcher artifact");
            std::hint::black_box(config.rule_count());
            started.elapsed()
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[hook-durable-artifact-recovery] bytes={} rules={} admissions=100 p99Nanos={}",
        bytes.len(),
        live.rule_count(),
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(50),
        "one-time durable Hook generation recovery must remain below 50 ms p99, observed {p99:?}"
    );
}
