use std::sync::{Arc, Barrier, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{ManagedHookConfigStatus, materialize};

fn test_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-managed-hook-config-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn hook_owned_refresh_covers_create_stale_and_warm_cycles() {
    let root = test_root("lifecycle");
    let path = root.join("hooks").join("config.toml");

    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Created));
    std::fs::write(&path, b"contractFingerprint = \"stale\"\n").expect("write stale config");
    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Refreshed));
    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Current));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read refreshed config"),
        agent_semantic_hook::default_client_config_template()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn concurrent_stale_refresh_is_lock_free_and_converges() {
    const WORKERS: usize = 16;
    let root = test_root("concurrent");
    let path = Arc::new(root.join("hooks").join("config.toml"));
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("create parent");
    std::fs::write(path.as_ref(), b"contractFingerprint = \"stale\"\n")
        .expect("write stale config");

    let barrier = Arc::new(Barrier::new(WORKERS));
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let workers = (0..WORKERS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let path = Arc::clone(&path);
            let sender = sender.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let _ = sender.send(materialize(path.as_ref()));
            })
        })
        .collect::<Vec<_>>();
    drop(sender);

    for _ in 0..WORKERS {
        let status = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("concurrent refresh must not deadlock")
            .expect("concurrent refresh succeeds");
        assert!(matches!(
            status,
            ManagedHookConfigStatus::Current | ManagedHookConfigStatus::Refreshed
        ));
    }
    for worker in workers {
        worker.join().expect("refresh worker joins");
    }
    eprintln!(
        "[hook-config-refresh] scenario=concurrent workers={WORKERS} elapsedMicros={}",
        started.elapsed().as_micros()
    );
    assert_eq!(
        std::fs::read_to_string(path.as_ref()).expect("read converged config"),
        agent_semantic_hook::default_client_config_template()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn refresh_failure_is_bounded_and_leaves_no_waiting_lock() {
    let root = test_root("failure");
    std::fs::create_dir_all(&root).expect("create root");
    let blocked_parent = root.join("not-a-directory");
    std::fs::write(&blocked_parent, b"file").expect("write blocking file");

    let started = Instant::now();
    let error = materialize(&blocked_parent.join("config.toml"))
        .expect_err("non-directory parent must fail closed");
    eprintln!(
        "[hook-config-refresh] scenario=failure elapsedMicros={}",
        started.elapsed().as_micros()
    );
    assert!(error.contains("failed to read managed hook config"));
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "failure path exceeded bounded local filesystem latency: {:?}",
        started.elapsed()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn warm_fingerprint_gate_stays_millisecond_scale() {
    const CYCLES: u32 = 128;
    let root = test_root("warm");
    let path = root.join("hooks").join("config.toml");
    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Created));

    let started = Instant::now();
    for _ in 0..CYCLES {
        assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Current));
    }
    let elapsed = started.elapsed();
    eprintln!(
        "[hook-config-refresh] scenario=warm cycles={CYCLES} elapsedMicros={} averageMicros={}",
        elapsed.as_micros(),
        elapsed.as_micros() / u128::from(CYCLES)
    );
    assert!(
        elapsed < Duration::from_millis(512),
        "warm hook config gate exceeded 4ms average: {elapsed:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hook_refresh_has_no_sync_recursion_edge() {
    let hook_runtime = include_str!("../../src/command/hook_runtime.rs");
    let hook_recovery = include_str!("../../src/command/hook_runtime_config_recovery.rs");
    let sync = include_str!("../../src/command/sync.rs");

    for forbidden in [
        "super::sync::",
        "sync_agent_configuration",
        "ensure_codex_agent_configuration",
    ] {
        assert!(
            !hook_runtime.contains(forbidden),
            "hook runtime reintroduced sync recursion edge `{forbidden}`"
        );
    }
    assert!(!sync.contains("managed_hook_config"));
    assert!(!sync.contains("hook_config_status"));
    for legacy in [
        "super::super::sync::",
        "`asp sync`",
        "AutoSync",
        "repaired-by-asp-sync",
    ] {
        assert!(
            !hook_recovery.contains(legacy),
            "hook recovery reintroduced sync recursion marker `{legacy}`"
        );
    }
}
