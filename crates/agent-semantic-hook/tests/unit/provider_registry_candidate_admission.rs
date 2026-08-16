#[test]
fn every_registered_language_is_lazily_admitted_by_its_typed_scope_descriptor() {
    let manifests = super::schema_registry_provider_manifests();
    assert!(!manifests.is_empty());
    for manifest in manifests {
        let candidate = if let Some(document) = manifest.document_resolution() {
            let extension = document
                .extensions
                .first()
                .expect("document provider extension")
                .trim_start_matches('.');
            std::path::PathBuf::from(format!("docs/fixture.{extension}"))
        } else if let Some(project) = manifest.project_resolution() {
            std::path::PathBuf::from(
                project
                    .entry_markers
                    .first()
                    .expect("programming provider entry marker"),
            )
        } else {
            continue;
        };
        assert!(
            super::registered_provider_matches_candidate_paths(
                manifest.language_id().as_str(),
                manifest.provider_id().as_str(),
                [candidate.as_path()],
            )
            .expect("registered provider admission"),
            "registered provider was not admitted by its own descriptor: language={}",
            manifest.language_id()
        );
        if let Some(project) = manifest.project_resolution() {
            let nested_marker = std::path::Path::new("nested").join(
                project
                    .entry_markers
                    .first()
                    .expect("programming provider entry marker"),
            );
            assert!(
                !super::registered_provider_matches_candidate_paths(
                    manifest.language_id().as_str(),
                    manifest.provider_id().as_str(),
                    [nested_marker.as_path()],
                )
                .expect("nested provider admission"),
                "registered provider admitted a nested marker as the project root: language={}",
                manifest.language_id()
            );
        }
        assert!(
            !super::registered_provider_matches_candidate_paths(
                manifest.language_id().as_str(),
                manifest.provider_id().as_str(),
                [std::path::Path::new("unrelated/no-match.bin")],
            )
            .expect("unrelated provider admission"),
            "registered provider admitted an unrelated candidate: language={}",
            manifest.language_id()
        );
    }
}
