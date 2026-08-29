use std::sync::{Arc, Barrier, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::{ManagedHookConfigStatus, materialize};

#[cfg(unix)]
fn current_thread_cpu_nanos() -> u128 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `time` is a valid writable timespec and the clock is process-local.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    assert_eq!(status, 0, "read current-thread CPU clock");
    (time.tv_sec as u128) * 1_000_000_000 + (time.tv_nsec as u128)
}

#[cfg(not(unix))]
fn current_thread_cpu_nanos() -> u128 {
    static STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    STARTED.get_or_init(Instant::now).elapsed().as_nanos()
}

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

fn write_sidecar(path: &std::path::Path, bytes: &[u8]) {
    let sidecar = path.with_file_name(format!(
        "{}.managed.sha256",
        path.file_name()
            .and_then(|name| name.to_str())
            .expect("config file name")
    ));
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    std::fs::write(sidecar, hex).expect("write sidecar");
}

#[test]
fn hook_owned_refresh_covers_create_stale_and_warm_cycles() {
    let root = test_root("lifecycle");
    let path = root.join("hooks").join("config.toml");

    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Created));
    let stale = b"contractFingerprint = \"stale\"\n";
    std::fs::write(&path, stale).expect("write stale config");
    write_sidecar(&path, stale);
    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Migrated));
    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Current));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read refreshed config"),
        agent_semantic_hook::default_client_config_template()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn legacy_fingerprint_recovers_missing_sidecar() {
    let root = test_root("legacy-sidecar-recovery");
    let path = root.join("hooks").join("config.toml");
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("create config parent");
    let mut legacy =
        toml::from_str::<toml::Value>(&agent_semantic_hook::default_client_config_template())
            .expect("parse managed template");
    legacy["contractFingerprint"] = toml::Value::String("hook-client-v1-legacy-managed".to_owned());
    std::fs::write(
        &path,
        toml::to_string(&legacy).expect("render legacy managed config"),
    )
    .expect("write legacy managed config");

    assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Migrated));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read recovered config"),
        agent_semantic_hook::default_client_config_template()
    );
    assert!(path.with_file_name("config.toml.managed.sha256").is_file());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn concurrent_stale_refresh_is_lock_free_and_converges() {
    const WORKERS: usize = 16;
    let root = test_root("concurrent");
    let path = Arc::new(root.join("hooks").join("config.toml"));
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("create parent");
    let stale = b"contractFingerprint = \"stale\"\n";
    std::fs::write(path.as_ref(), stale).expect("write stale config");
    write_sidecar(path.as_ref(), stale);

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
            ManagedHookConfigStatus::Current | ManagedHookConfigStatus::Migrated
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

    let cpu_started_nanos = current_thread_cpu_nanos();
    for _ in 0..CYCLES {
        assert_eq!(materialize(&path), Ok(ManagedHookConfigStatus::Current));
    }
    let elapsed_cpu_nanos = current_thread_cpu_nanos() - cpu_started_nanos;
    eprintln!(
        "[hook-config-refresh] scenario=warm cycles={CYCLES} cpuMicros={} averageCpuMicros={}",
        elapsed_cpu_nanos / 1_000,
        elapsed_cpu_nanos / 1_000 / u128::from(CYCLES)
    );
    assert!(
        elapsed_cpu_nanos < Duration::from_millis(512).as_nanos(),
        "warm hook config gate exceeded 4ms average of current-thread CPU: {}ns",
        elapsed_cpu_nanos / u128::from(CYCLES)
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hook_refresh_has_no_sync_recursion_edge() {
    let hook_runtime = include_str!("../../src/command/hook_runtime.rs");

    for forbidden in [
        "super::sync::",
        "sync_agent_configuration",
        "ensure_codex_agent_configuration",
        "super::run_protocol_command",
    ] {
        assert!(
            !hook_runtime.contains(forbidden),
            "hook runtime reintroduced sync recursion edge `{forbidden}`"
        );
    }
    for legacy in ["`asp sync`", "AutoSync", "repaired-by-asp-sync"] {
        assert!(
            !hook_runtime.contains(legacy),
            "hook recovery reintroduced sync recursion marker `{legacy}`"
        );
    }
}
