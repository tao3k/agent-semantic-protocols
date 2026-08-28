use super::{PROBE_TIMEOUT, observe_one};
use agent_semantic_hook::ReaderProbeAccess;

#[cfg(target_os = "macos")]
fn fixture() -> String {
    let root = super::materialize_probe_root().expect("materialize probe root");
    let bytes = crate::reader_probe_fixture_bytes();
    let path = root.join(format!("fixture-{}", blake3::hash(bytes).to_hex()));
    if !path.exists() {
        std::fs::write(&path, bytes).expect("materialize probe fixture");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500))
            .expect("make probe fixture executable");
    }
    path.to_string_lossy().into_owned()
}

#[test]
fn explicit_diagnostic_timeout_kills_and_reaps_the_fixture() {
    #[cfg(target_os = "macos")]
    {
        let started = std::time::Instant::now();
        let observation = observe_one(
            &[fixture(), "hang".to_owned(), "fixture.rs".to_owned()],
            "fixture.rs".to_owned(),
        );
        assert_eq!(observation.access, ReaderProbeAccess::Unknown);
        assert_eq!(observation.terminal, "probe-timeout");
        assert!(observation.cleanup_verified, "observation={observation:?}");
        assert!(started.elapsed() < PROBE_TIMEOUT + std::time::Duration::from_millis(200));
    }
}

#[test]
fn non_macos_backend_fails_closed() {
    #[cfg(not(target_os = "macos"))]
    {
        let observation = observe_one(&[], "fixture.rs".to_owned());
        assert_eq!(observation.access, ReaderProbeAccess::Unknown);
        assert_eq!(observation.terminal, "unsupported-platform");
    }
}

#[test]
fn profile_sentinel_is_reused_as_one_stable_inode() {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::MetadataExt;

        let root = super::materialize_probe_root().expect("probe root");
        let first = super::materialize_profile_sentinel(&root, "src/lib.rs")
            .expect("first profile sentinel");
        let first_inode = std::fs::metadata(&first).expect("first metadata").ino();
        let second = super::materialize_profile_sentinel(&root, "another/path.rs")
            .expect("second profile sentinel");
        let second_inode = std::fs::metadata(&second).expect("second metadata").ino();
        assert_eq!(first, second);
        assert_eq!(first_inode, second_inode);
    }
}

#[test]
fn concurrent_first_materialization_is_atomic_and_content_verified() {
    #[cfg(target_os = "macos")]
    {
        let root = tempfile::tempdir().expect("isolated Reader materialization root");
        let workers = (0..32)
            .map(|_| {
                let root = root.path().to_owned();
                std::thread::spawn(move || {
                    let interposer =
                        super::materialize_interposer(&root).expect("materialize interposer");
                    let sentinel = super::materialize_profile_sentinel(&root, "src/lib.rs")
                        .expect("materialize sentinel");
                    (interposer, sentinel)
                })
            })
            .collect::<Vec<_>>();
        let expected_interposer = blake3::hash(crate::reader_probe_interposer_bytes()).to_hex();
        for worker in workers {
            let (interposer, sentinel) = worker.join().expect("materialization worker");
            assert_eq!(
                blake3::hash(&std::fs::read(interposer).expect("read interposer")).to_hex(),
                expected_interposer
            );
            assert_eq!(
                std::fs::metadata(sentinel)
                    .expect("sentinel metadata")
                    .len(),
                0
            );
        }
    }
}
