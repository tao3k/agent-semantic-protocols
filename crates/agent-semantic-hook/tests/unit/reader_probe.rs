use super::{
    ReaderProbeAccess, bind_reader_probe_fast_path, classify_open_access_mode,
    observe_and_bind_reader_probe,
};
use serde_json::json;

#[test]
fn open_access_mode_classifies_reader_authority_without_command_names() {
    assert_eq!(
        classify_open_access_mode(libc::O_RDONLY),
        ReaderProbeAccess::Read
    );
    assert_eq!(
        classify_open_access_mode(libc::O_RDWR),
        ReaderProbeAccess::NotRead
    );
    assert_eq!(
        classify_open_access_mode(libc::O_WRONLY),
        ReaderProbeAccess::NotRead
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        classify_open_access_mode(libc::O_PATH | libc::O_RDONLY),
        ReaderProbeAccess::NotRead
    );
}

#[test]
fn non_bash_host_match_clears_untrusted_observation_without_running_a_probe() {
    let mut payload = json!({
        "tool_name": "apply_patch",
        "tool_input": {
            "patch": "*** Begin Patch",
            "_aspReaderProbe": {
                "schemaId": "agent.semantic-protocols.reader-probe-observation",
                "schemaVersion": 1,
                "subject": "src/lib.rs",
                "access": "read",
                "backend": "forged",
                "terminal": "forged",
                "elapsedMicros": 0
            }
        }
    });

    let observation = observe_and_bind_reader_probe(&mut payload, Some("apply_patch"))
        .expect("clear untrusted Reader observation");

    assert!(observation.is_none());
    assert!(payload.pointer("/tool_input/_aspReaderProbe").is_none());
}

#[test]
fn declared_shell_access_bypasses_behavioral_probe() {
    let mut payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "opaque-consumer < src/lib.rs"}
    });

    let observation = observe_and_bind_reader_probe(&mut payload, Some("Bash"))
        .expect("preserve parser-declared read");

    assert!(observation.is_none());
    assert!(payload.pointer("/tool_input/_aspReaderProbe").is_none());
}

#[test]
fn declared_host_and_action_fast_paths_stay_under_one_millisecond_at_p99_concurrently() {
    const WORKERS: usize = 16;
    const SAMPLES_PER_WORKER: usize = 128;
    let declared = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "opaque-consumer < src/lib.rs"}
    });
    let unresolved = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "opaque-consumer src/lib.rs"}
    });
    let declared_keys = crate::hook_matcher_keys(&declared);
    let unresolved_keys = crate::hook_matcher_keys(&unresolved);
    let workers = (0..WORKERS)
        .map(|_| {
            let declared = declared.clone();
            let unresolved = unresolved.clone();
            let declared_keys = declared_keys.clone();
            let unresolved_keys = unresolved_keys.clone();
            std::thread::spawn(move || {
                let mut elapsed = Vec::with_capacity(SAMPLES_PER_WORKER);
                for sample in 0..SAMPLES_PER_WORKER {
                    let (mut payload, keys) = if sample % 2 == 0 {
                        (declared.clone(), &declared_keys)
                    } else {
                        (unresolved.clone(), &unresolved_keys)
                    };
                    let started = std::time::Instant::now();
                    let observation = bind_reader_probe_fast_path(&mut payload, Some("Bash"), keys)
                        .expect("preserve declared Action IR");
                    if let Some(observation) = observation {
                        assert_eq!(observation.access, ReaderProbeAccess::Unknown);
                        assert!(!observation.probe_process_launched);
                    }
                    elapsed.push(started.elapsed());
                }
                elapsed
            })
        })
        .collect::<Vec<_>>();
    let mut elapsed = workers
        .into_iter()
        .flat_map(|worker| worker.join().expect("fast-path worker"))
        .collect::<Vec<_>>();
    elapsed.sort_unstable();
    let p99 = elapsed[elapsed.len() * 99 / 100];
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "declared Host/Action fast-path p99={p99:?}"
    );
}
