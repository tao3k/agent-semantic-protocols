// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[cfg(target_os = "macos")]
use super::PROBE_COLD_TIMEOUT;
use super::observe_one;
use agent_semantic_hook::ReaderProbeAccess;

#[cfg(target_os = "macos")]
static COLD_PROBE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(target_os = "macos")]
fn cold_probe_test_guard() -> std::sync::MutexGuard<'static, ()> {
    COLD_PROBE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

#[cfg(target_os = "macos")]
fn cache_fixture() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt as _;

    let cache = tempfile::tempdir().expect("Reader behavior cache");
    std::fs::set_permissions(cache.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private Reader behavior cache");
    cache
}

#[test]
fn explicit_diagnostic_timeout_kills_and_reaps_the_fixture() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let cache = cache_fixture();
        let started = std::time::Instant::now();
        let observation = observe_one(
            &[fixture(), "hang".to_owned(), "fixture.rs".to_owned()],
            "fixture.rs".to_owned(),
            &[],
            Some(cache.path()),
        );
        assert_eq!(observation.access, ReaderProbeAccess::Unknown);
        assert_eq!(observation.terminal, "probe-timeout");
        assert!(observation.cleanup_verified, "observation={observation:?}");
        assert!(started.elapsed() < PROBE_COLD_TIMEOUT + std::time::Duration::from_millis(200));
    }
}

#[test]
fn non_macos_backend_fails_closed() {
    #[cfg(not(target_os = "macos"))]
    {
        let observation = observe_one(&[], "fixture.rs".to_owned(), &[], None);
        assert_eq!(observation.access, ReaderProbeAccess::Unknown);
        assert_eq!(observation.terminal, "unsupported-platform");
    }
}

#[test]
fn profile_permission_sentinels_are_reused_as_stable_inodes() {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::MetadataExt;

        let root = super::materialize_probe_root().expect("probe root");
        let first = super::materialize_profile_sentinels(&root, "src/lib.rs")
            .expect("first profile sentinels");
        let first_readable_inode = std::fs::metadata(&first.readable)
            .expect("first readable metadata")
            .ino();
        let first_denied_inode = std::fs::metadata(&first.denied)
            .expect("first denied metadata")
            .ino();
        let second = super::materialize_profile_sentinels(&root, "another/path.rs")
            .expect("second profile sentinels");
        assert_eq!(first.readable, second.readable);
        assert_eq!(first.denied, second.denied);
        assert_eq!(
            first_readable_inode,
            std::fs::metadata(second.readable).unwrap().ino()
        );
        assert_eq!(
            first_denied_inode,
            std::fs::metadata(second.denied).unwrap().ino()
        );
    }
}

#[test]
fn concurrent_permission_sentinel_materialization_is_stable() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let root = tempfile::tempdir().expect("isolated Reader materialization root");
        let workers = (0..32)
            .map(|_| {
                let root = root.path().to_owned();
                std::thread::spawn(move || {
                    super::materialize_profile_sentinels(&root, "src/lib.rs")
                        .expect("materialize permission sentinels")
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            use std::os::unix::fs::PermissionsExt as _;

            let sentinels = worker.join().expect("materialization worker");
            assert_eq!(
                std::fs::metadata(&sentinels.readable)
                    .expect("readable sentinel metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o400
            );
            assert_eq!(
                std::fs::metadata(&sentinels.denied)
                    .expect("denied sentinel metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o000
            );
            assert_eq!(std::fs::metadata(sentinels.readable).unwrap().len(), 0);
            assert_eq!(std::fs::metadata(sentinels.denied).unwrap().len(), 0);
        }
    }
}

#[test]
fn static_catalog_is_a_process_free_reader_fact() {
    let observation = observe_one(
        &["/opt/tools/head".to_owned(), "src/lib.rs".to_owned()],
        "src/lib.rs".to_owned(),
        &[vec!["head".to_owned()]],
        None,
    );
    assert_eq!(observation.access, ReaderProbeAccess::Read);
    assert_eq!(observation.terminal, "reader-behavior-catalog-hit");
    assert_eq!(observation.backend, "hook-policy-bundle-reader-catalog");
    assert!(!observation.probe_process_launched);
    assert!(!observation.cache_hit);
}

#[test]
fn static_catalog_requires_the_complete_declared_prefix() {
    let patterns = [vec!["sed".to_owned(), "-n".to_owned()]];
    let read = observe_one(
        &["sed".to_owned(), "-n".to_owned(), "src/lib.rs".to_owned()],
        "src/lib.rs".to_owned(),
        &patterns,
        None,
    );
    assert_eq!(read.access, ReaderProbeAccess::Read);
    assert!(!read.probe_process_launched);

    let edit = observe_one(
        &["sed".to_owned(), "-i".to_owned(), "src/lib.rs".to_owned()],
        "src/lib.rs".to_owned(),
        &patterns,
        None,
    );
    assert_ne!(edit.terminal, "reader-behavior-catalog-hit");
}

#[test]
fn static_catalog_matches_wrapped_absolute_executable_by_basename() {
    let tokens = [
        ".devenv/devenv-profile-exec".to_owned(),
        "/usr/bin/git".to_owned(),
        "show".to_owned(),
        "HEAD:src/lib.rs".to_owned(),
    ];
    let patterns = [vec![
        "git".to_owned(),
        "show".to_owned(),
        "*:*.?*".to_owned(),
    ]];
    assert!(super::static_reader_behavior_matches(
        &tokens,
        "HEAD:src/lib.rs",
        &patterns,
        true,
    ));
}

#[test]
fn static_catalog_does_not_scan_executable_like_tokens_after_subject() {
    let tokens = [
        "future-wrapper".to_owned(),
        "src/lib.rs".to_owned(),
        "/usr/bin/git".to_owned(),
        "show".to_owned(),
        "HEAD:other.rs".to_owned(),
    ];
    let patterns = [vec![
        "git".to_owned(),
        "show".to_owned(),
        "*:*.?*".to_owned(),
    ]];
    assert!(!super::static_reader_behavior_matches(
        &tokens,
        "src/lib.rs",
        &patterns,
        true,
    ));
}

#[test]
fn verified_reader_is_reused_from_state_home_without_a_second_process() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let cache = cache_fixture();
        let tokens = vec![fixture(), "read".to_owned(), "fixture.rs".to_owned()];
        let first = observe_one(&tokens, "fixture.rs".to_owned(), &[], Some(cache.path()));
        assert_eq!(first.access, ReaderProbeAccess::Read, "{first:?}");
        assert!(first.probe_process_launched, "{first:?}");
        assert!(!first.cache_hit);
        let second = observe_one(&tokens, "fixture.rs".to_owned(), &[], Some(cache.path()));
        assert_eq!(second.access, ReaderProbeAccess::Read, "{second:?}");
        assert!(!second.probe_process_launched, "{second:?}");
        assert!(second.cache_hit, "{second:?}");
        assert_eq!(first.behavior_key, second.behavior_key);
    }
}

#[test]
fn concurrent_cold_miss_launches_exactly_one_probe() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let probe_root = super::materialize_probe_root().expect("probe root");
        super::materialize_profile_sentinels(&probe_root, "fixture.rs")
            .expect("prime permission sentinels");
        let cache = cache_fixture();
        super::prepare_dynamic_cache_root(cache.path()).expect("prime cache namespace");
        let fixture_path = fixture();
        let start = std::sync::Arc::new(std::sync::Barrier::new(33));
        let workers = (0..32)
            .map(|_| {
                let cache = cache.path().to_owned();
                let start = std::sync::Arc::clone(&start);
                let tokens = vec![
                    fixture_path.clone(),
                    "read".to_owned(),
                    "fixture.rs".to_owned(),
                ];
                std::thread::spawn(move || {
                    start.wait();
                    observe_one(&tokens, "fixture.rs".to_owned(), &[], Some(&cache))
                })
            })
            .collect::<Vec<_>>();
        start.wait();
        let observations = workers
            .into_iter()
            .map(|worker| worker.join().expect("Reader worker"))
            .collect::<Vec<_>>();
        assert_eq!(
            observations
                .iter()
                .filter(|observation| observation.probe_process_launched)
                .count(),
            1,
            "observations={observations:#?}"
        );
        assert!(
            observations.iter().all(|observation| {
                observation.access == ReaderProbeAccess::Read
                    || (observation.access == ReaderProbeAccess::Unknown
                        && observation.terminal == "reader-behavior-cache-wait-timeout")
            }),
            "a busy cold slot may return only the typed non-queue timeout: observations={observations:#?}"
        );
        let eventual_read = observe_one(
            &[fixture_path, "read".to_owned(), "fixture.rs".to_owned()],
            "fixture.rs".to_owned(),
            &[],
            Some(cache.path()),
        );
        assert_eq!(
            eventual_read.access,
            ReaderProbeAccess::Read,
            "{eventual_read:?}"
        );
        let after_commit = observe_one(
            &[fixture(), "read".to_owned(), "fixture.rs".to_owned()],
            "fixture.rs".to_owned(),
            &[],
            Some(cache.path()),
        );
        assert_eq!(
            after_commit.access,
            ReaderProbeAccess::Read,
            "{after_commit:?}"
        );
        assert!(after_commit.cache_hit, "{after_commit:?}");
        assert!(!after_commit.probe_process_launched, "{after_commit:?}");
    }
}

#[test]
fn write_behavior_is_never_published_as_reader_cache() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let cache = cache_fixture();
        let tokens = vec![fixture(), "write".to_owned(), "fixture.rs".to_owned()];
        for _ in 0..2 {
            let observation =
                observe_one(&tokens, "fixture.rs".to_owned(), &[], Some(cache.path()));
            assert_ne!(
                observation.access,
                ReaderProbeAccess::Read,
                "{observation:?}"
            );
            assert!(matches!(observation.access, ReaderProbeAccess::Unknown));
            assert!(!observation.cache_hit);
        }
    }
}

#[test]
fn timeout_is_never_published_as_reader_cache() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let cache = cache_fixture();
        let tokens = vec![fixture(), "hang".to_owned(), "fixture.rs".to_owned()];
        for _ in 0..2 {
            let observation =
                observe_one(&tokens, "fixture.rs".to_owned(), &[], Some(cache.path()));
            assert_eq!(observation.access, ReaderProbeAccess::Unknown);
            assert_eq!(observation.terminal, "probe-timeout");
            assert!(observation.probe_process_launched);
            assert!(!observation.cache_hit);
            assert!(observation.cleanup_verified);
        }
    }
}

#[test]
fn corrupt_positive_record_is_rejected_and_atomically_replaced() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        use std::os::unix::fs::PermissionsExt as _;

        let cache = cache_fixture();
        super::prepare_dynamic_cache_root(cache.path()).expect("prepare cache namespace");
        let key = blake3::hash(b"corrupt-record-fixture");
        super::publish_dynamic_cache_record(cache.path(), &key).expect("publish positive record");
        assert!(super::dynamic_cache_hit(cache.path(), &key));
        let record = super::dynamic_cache_record_path(cache.path(), &key);
        std::fs::write(&record, b"corrupt").expect("corrupt record");
        std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600))
            .expect("private corrupt record");
        super::clear_process_positive_cache();
        assert!(!super::dynamic_cache_hit(cache.path(), &key));
        super::publish_dynamic_cache_record(cache.path(), &key).expect("replace corrupt record");
        assert!(super::dynamic_cache_hit(cache.path(), &key));
    }
}

#[test]
fn state_homes_and_argument_shapes_do_not_share_dynamic_authority() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let first_home = cache_fixture();
        let second_home = cache_fixture();
        let read = vec![fixture(), "read".to_owned(), "fixture.rs".to_owned()];
        let read_alt = vec![
            fixture(),
            "read".to_owned(),
            "--alternate".to_owned(),
            "fixture.rs".to_owned(),
        ];

        let first = observe_one(&read, "fixture.rs".to_owned(), &[], Some(first_home.path()));
        let other_home = observe_one(
            &read,
            "fixture.rs".to_owned(),
            &[],
            Some(second_home.path()),
        );
        let other_shape = observe_one(
            &read_alt,
            "fixture.rs".to_owned(),
            &[],
            Some(first_home.path()),
        );
        assert!(first.probe_process_launched, "{first:?}");
        assert!(other_home.probe_process_launched, "{other_home:?}");
        assert!(other_shape.probe_process_launched, "{other_shape:?}");
        assert_ne!(first.behavior_key, other_shape.behavior_key);
    }
}

#[test]
fn dynamic_catalog_retention_is_bounded() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = cold_probe_test_guard();
        let cache = cache_fixture();
        super::prepare_dynamic_cache_root(cache.path()).expect("prepare cache");
        for index in 0_u64..300 {
            let key = blake3::hash(&index.to_le_bytes());
            super::publish_dynamic_cache_record(cache.path(), &key).expect("publish record");
        }
        let records = std::fs::read_dir(cache.path())
            .expect("read cache")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("bin")
            })
            .count();
        assert_eq!(records, super::DYNAMIC_CACHE_MAX_RECORDS);
    }
}
