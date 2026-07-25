use agent_semantic_config::{LanguageId, ProviderId};
use agent_semantic_content_identity::active_artifact_merkle_v1::ActiveArtifactKindV1;
use agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1;
use agent_semantic_hook::{
    ActiveAspArtifactInput, active_provider_artifact_input, materialize_active_asp_artifact_receipt,
};
use std::{
    ffi::OsString,
    fs,
    path::Path,
    sync::MutexGuard,
    time::{SystemTime, UNIX_EPOCH},
};

struct AspStateHomeGuard {
    previous: Option<OsString>,
    _lock: MutexGuard<'static, ()>,
}

impl AspStateHomeGuard {
    fn activate(state_home: &Path) -> Self {
        let lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = std::env::var_os("ASP_STATE_HOME");
        // SAFETY: access to ASP_STATE_HOME is serialized for this integration test.
        unsafe {
            std::env::set_var("ASP_STATE_HOME", state_home);
        }
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for AspStateHomeGuard {
    fn drop(&mut self) {
        // SAFETY: this guard retains the process-local environment lock.
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("ASP_STATE_HOME", previous);
            } else {
                std::env::remove_var("ASP_STATE_HOME");
            }
        }
    }
}

#[test]
fn unchanged_provider_artifacts_are_zero_byte_read_and_zero_write() {
    let root = std::env::temp_dir().join(format!(
        "asp-active-artifact-warm-path-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create test root");
    let binary_path = root.join("asp");
    let activation_path = root.join("activation.json");
    let provider_path = root.join("provider");
    let binary_bytes = b"asp-test-binary";
    fs::write(&binary_path, binary_bytes).expect("write binary");
    fs::write(&activation_path, b"{}").expect("write activation");
    fs::write(&provider_path, vec![0x5a; 16 * 1024 * 1024]).expect("write provider");
    let binary_digest = blake3_content_digest_v1(binary_bytes);
    let provider_digest = agent_semantic_content_identity::file_content_digest_v1(&provider_path)
        .expect("provider digest");
    let artifacts = [ActiveAspArtifactInput {
        logical_path: "providers/rust/rs-harness".to_string(),
        artifact_kind: ActiveArtifactKindV1::ProviderBinary,
        materialized_path: provider_path,
        artifact_digest: provider_digest,
    }];

    let cold = materialize_active_asp_artifact_receipt(
        &binary_path,
        binary_digest.as_str(),
        &activation_path,
        &artifacts,
    )
    .expect("cold materialization");
    assert_eq!(cold.artifact_byte_reads, 0);
    assert_eq!(cold.artifact_bytes_read, 0);
    assert_eq!(cold.receipt_writes, 1);

    let started = std::time::Instant::now();
    let warm = materialize_active_asp_artifact_receipt(
        &binary_path,
        binary_digest.as_str(),
        &activation_path,
        &artifacts,
    )
    .expect("warm materialization");
    let elapsed = started.elapsed();
    assert_eq!(warm.artifact_byte_reads, 0);
    assert_eq!(warm.artifact_bytes_read, 0);
    assert_eq!(warm.receipt_writes, 0);
    assert_eq!(warm.receipt, cold.receipt);
    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "warm materialization must remain millisecond-scale, elapsed={elapsed:?}"
    );

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn provider_install_receipt_metadata_is_required_and_drift_fails_closed() {
    let root = std::env::temp_dir().join(format!(
        "asp-provider-install-receipt-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create project root");
    let _state_home_guard = AspStateHomeGuard::activate(&root.join("state-home"));
    let state_paths =
        agent_semantic_runtime::project_state_paths(&root).expect("resolve project state paths");
    let provider_lock_dir = state_paths.provider_lock_dir;
    let provider_path = root.join("runtime/bin/rs-harness");
    fs::create_dir_all(&provider_lock_dir).expect("create provider lock dir");
    fs::create_dir_all(provider_path.parent().expect("provider parent"))
        .expect("create provider bin dir");
    fs::write(&provider_path, b"provider-v1").expect("write provider");
    let provider_digest = agent_semantic_content_identity::file_content_digest_v1(&provider_path)
        .expect("provider digest");
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&provider_path)
            .expect("metadata digest");
    let lock_path = provider_lock_dir.join("rust.lock.toml");
    fs::write(
        &lock_path,
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"rs-harness\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{provider_digest}\"\ninstalledEntrypointMetadataDigest = \"{metadata_digest}\"\n",
            provider_path.display()
        ),
    )
    .expect("write provider lock");

    let language_id = LanguageId::new("rust");
    let provider_id = ProviderId::new("rs-harness");
    let input =
        active_provider_artifact_input(&root, &language_id, &provider_id, provider_path.clone())
            .expect("consume provider receipt");
    assert_eq!(input.artifact_digest, provider_digest);

    fs::write(&provider_path, b"provider-drift").expect("drift provider");
    let error =
        active_provider_artifact_input(&root, &language_id, &provider_id, provider_path.clone())
            .expect_err("metadata drift must fail closed");
    assert!(
        error.contains("provider install receipt metadata drift"),
        "{error}"
    );

    fs::write(
        &lock_path,
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"rs-harness\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{provider_digest}\"\n",
            provider_path.display()
        ),
    )
    .expect("write incomplete provider lock");
    let error = active_provider_artifact_input(&root, &language_id, &provider_id, provider_path)
        .expect_err("missing metadata digest must fail closed");
    assert!(error.contains("failed to parse"), "{error}");

    fs::remove_dir_all(root).expect("remove test root");
}
