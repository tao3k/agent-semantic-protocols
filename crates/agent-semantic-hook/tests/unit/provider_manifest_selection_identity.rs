use super::{
    ProviderCommandSelection, build_default_activation_from_selections, provider_manifests,
};

#[test]
fn activation_reuses_selection_manifest_identity_in_milliseconds() {
    let executable = std::env::current_exe()
        .expect("resolve current test executable")
        .display()
        .to_string();
    let selections = provider_manifests()
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
            native_library: manifest.native_library.clone(),
            provider_command_prefix: vec![executable.clone()],
        })
        .collect::<Vec<_>>();

    let started = std::time::Instant::now();
    let activation = build_default_activation_from_selections(
        std::path::Path::new("/activation-selection-identity-gate"),
        &selections,
    )
    .expect("build activation from typed provider selections");
    let elapsed = started.elapsed();

    for provider in &activation.providers {
        let selection = selections
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
fn activation_preserves_c_family_native_library_descriptor() {
    let manifests = provider_manifests();
    let manifest = manifests
        .iter()
        .find(|manifest| manifest.language_id == "c")
        .expect("C provider manifest");
    let selection = ProviderCommandSelection {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: super::provider_manifest_digest(manifest).expect("manifest digest"),
        execution_command_digest: "sha256:test-c-native-command".to_string(),
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary: manifest.binary.clone(),
        execution: manifest.execution,
        native_library: manifest.native_library.clone(),
        provider_command_prefix: Vec::new(),
    };
    let activation = build_default_activation_from_selections(
        std::path::Path::new("/activation-c-native-descriptor-gate"),
        &[selection],
    )
    .expect("build C activation");
    let serialized = serde_json::to_string(&activation).expect("serialize C activation");

    let runtime = crate::parse_activation(&serialized, &manifests).expect("parse C activation");

    assert_eq!(runtime.providers.len(), 1);
    assert_eq!(
        runtime.providers[0].native_library, manifest.native_library,
        "activated C provider must retain the manifest-locked native library descriptor"
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
        native_library: manifest.native_library.clone(),
        provider_command_prefix: vec![
            std::path::Path::new("/state-home/runtime/bin")
                .join(selected_binary)
                .display()
                .to_string(),
        ],
    };
    let activation = build_default_activation_from_selections(
        std::path::Path::new("/activation-selected-basename-gate"),
        &[selection],
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
