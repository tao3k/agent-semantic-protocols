use super::{
    DefaultActivationSelections, ProviderCommandSelection, RuntimeBinarySelectionV1,
    build_default_activation_from_selections, provider_manifests,
    reuse_asp_binary_selection_from_active_receipt,
};

fn test_graph_turbo_selection() -> RuntimeBinarySelectionV1 {
    RuntimeBinarySelectionV1::new(
        "/producer-owned/runtime/bin/asp".to_string(),
        "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
    )
    .expect("valid producer-owned ASP binary identity")
}

#[test]
fn runtime_binary_identity_reuses_active_receipt_and_fails_closed_on_drift() {
    let root = std::env::temp_dir().join(format!(
        "asp-runtime-binary-receipt-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create receipt fixture");
    let binary = root.join("asp");
    let activation = root.join("activation.json");
    std::fs::write(&binary, b"asp-runtime-v1").expect("write runtime binary");
    std::fs::write(
        &activation,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": "asp.hook-activation.v1",
            "schemaVersion": "1",
            "schemaAuthority": "test",
            "protocolId": "agent.semantic-protocols.hook",
            "protocolVersion": "1",
            "projectRoot": root.display().to_string(),
            "generatedBy": {"runtime": "test", "version": "1"},
            "rankers": [],
            "providers": []
        }))
        .expect("encode activation"),
    )
    .expect("write activation");
    let digest = agent_semantic_content_identity::file_content_digest_v1(&binary)
        .expect("runtime binary digest");
    crate::materialize_active_asp_artifact_receipt(&binary, &digest, &activation, &[])
        .expect("materialize active artifact receipt");
    crate::verify_active_asp_artifact_receipt(&activation, &[&binary])
        .expect("verify active artifact receipt fixture");

    let started = std::time::Instant::now();
    let selection = reuse_asp_binary_selection_from_active_receipt(&binary, &activation)
        .expect("reuse verified active artifact receipt");
    let elapsed = started.elapsed();
    assert_eq!(selection.content_digest(), digest);
    assert!(
        elapsed < std::time::Duration::from_millis(25),
        "receipt-backed runtime identity exceeded the 25ms gate: {elapsed:?}"
    );

    std::fs::write(&binary, b"asp-runtime-v2").expect("drift runtime binary");
    assert!(
        reuse_asp_binary_selection_from_active_receipt(&binary, &activation).is_none(),
        "drifted runtime binary must not reuse the published content identity"
    );
    std::fs::remove_dir_all(root).expect("remove receipt fixture");
}

#[test]
fn activation_reuses_selection_manifest_identity_in_milliseconds() {
    let executable = std::env::current_exe()
        .expect("resolve current test executable")
        .display()
        .to_string();
    let providers = provider_manifests()
        .into_iter()
        .enumerate()
        .map(|(index, manifest)| ProviderCommandSelection {
            manifest_id: manifest.manifest_id.clone(),
            manifest_digest: format!("sha256:{index:064x}"),
            execution_command_digest: format!("sha256:{:064x}", index + 1),
            language_id: manifest.language_id.clone(),
            provider_id: manifest.provider_id.clone(),
            binary: if index == 0 {
                "custom-provider-basename".to_string()
            } else {
                manifest.binary.clone()
            },
            execution: manifest.execution,
            provider_command_prefix: vec![executable.clone()],
        })
        .collect::<Vec<_>>();
    let graph_turbo = test_graph_turbo_selection();
    let selections = DefaultActivationSelections::new(providers, graph_turbo.clone());

    let started = std::time::Instant::now();
    let activation = build_default_activation_from_selections(
        std::path::Path::new("/activation-selection-identity-gate"),
        &selections,
    )
    .expect("build activation from typed provider selections");
    let elapsed = started.elapsed();

    for provider in &activation.providers {
        let selection = selections
            .providers()
            .iter()
            .find(|selection| selection.manifest_id == provider.manifest_id)
            .expect("activated provider retains its typed selection");
        assert_eq!(
            provider.manifest_digest, selection.manifest_digest,
            "activation must consume the producer-owned manifest identity without re-hashing"
        );
        assert_eq!(
            provider.execution_command_digest, selection.execution_command_digest,
            "activation must consume the producer-owned execution identity without reading provider bytes"
        );
        assert_eq!(
            provider.binary, selection.binary,
            "activation must preserve the selected logical provider basename"
        );
    }
    let ranker = activation
        .rankers
        .first()
        .expect("built-in graph-turbo ranker");
    assert_eq!(ranker.binary, graph_turbo.binary());
    assert_eq!(ranker.content_digest, graph_turbo.content_digest());
    assert_eq!(
        ranker.artifact_metadata_digest,
        graph_turbo.artifact_metadata_digest()
    );
    assert!(
        elapsed < std::time::Duration::from_millis(250),
        "typed activation materialization exceeded the 250ms gate: {elapsed:?}"
    );
    eprintln!(
        "[activation-selection-perf] providers={} elapsedMicros={} budgetMicros=250000",
        activation.providers.len(),
        elapsed.as_micros()
    );
}

#[test]
fn activation_parser_preserves_configured_logical_basename() {
    let manifests = provider_manifests();
    let manifest = manifests.first().expect("builtin provider manifest");
    let selected_binary = "custom-provider-basename";
    let selection = ProviderCommandSelection {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: super::provider_manifest_digest(manifest).expect("manifest digest"),
        execution_command_digest: "sha256:test-execution-command".to_string(),
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary: selected_binary.to_string(),
        execution: manifest.execution,
        provider_command_prefix: vec![
            std::path::Path::new("/state-home/runtime/bin")
                .join(selected_binary)
                .display()
                .to_string(),
        ],
    };
    let selections =
        DefaultActivationSelections::new(vec![selection], test_graph_turbo_selection());
    let activation = build_default_activation_from_selections(
        std::path::Path::new("/activation-selected-basename-gate"),
        &selections,
    )
    .expect("build activation with configured logical basename");
    let serialized = serde_json::to_string(&activation).expect("serialize activation");

    let runtime =
        crate::parse_activation(&serialized, &manifests).expect("parse selected basename");
    assert_eq!(runtime.providers[0].binary, selected_binary);
    assert!(
        runtime.providers[0].provider_command_prefix.is_empty(),
        "State Home v1 activation must not persist the resolved provider path"
    );
}
