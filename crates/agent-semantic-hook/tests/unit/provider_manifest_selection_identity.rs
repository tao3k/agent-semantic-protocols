use super::{
    StaticActivationSelections, StaticProviderSelection, build_default_activation_from_selections,
    provider_manifests,
};

#[test]
fn activation_reuses_selection_manifest_identity_in_milliseconds() {
    let providers = provider_manifests()
        .into_iter()
        .filter(|manifest| manifest.document_resolution().is_some())
        .enumerate()
        .map(|(index, manifest)| StaticProviderSelection {
            manifest_id: manifest.manifest_id.clone(),
            manifest_digest: format!("sha256:{index:064x}"),
            language_id: manifest.language_id.clone(),
            provider_id: manifest.provider_id.clone(),
        })
        .collect::<Vec<_>>();
    let selections = StaticActivationSelections::new(providers);
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repository workspace root");

    let started = std::time::Instant::now();
    let activation = build_default_activation_from_selections(workspace_root, &selections)
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
    }
    let ranker = activation
        .rankers
        .first()
        .expect("built-in graph-turbo ranker");
    assert_eq!(ranker.capability_id, "graph-turbo");
    assert_eq!(ranker.argv_prefix, ["graph", "render"]);
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
    let manifest = manifests
        .iter()
        .find(|manifest| manifest.document_resolution().is_some())
        .expect("built-in document provider manifest");
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repository workspace root");
    let selection = StaticProviderSelection {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: super::provider_manifest_digest(manifest).expect("manifest digest"),
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
    };
    let selections = StaticActivationSelections::new(vec![selection]);
    let activation = build_default_activation_from_selections(workspace_root, &selections)
        .expect("build activation with configured logical basename");
    let serialized = serde_json::to_string(&activation).expect("serialize activation");

    let runtime =
        crate::parse_activation(&serialized, &manifests).expect("parse selected basename");
    assert_eq!(runtime.providers[0].provider_id, manifest.provider_id);
    assert!(!serialized.contains("providerCommandPrefix"));
}
