use super::{
    GRAPH_TURBO_BUNDLE_NAME, GRAPH_TURBO_CONFIG_NAME, GRAPH_TURBO_EXECUTABLE_NAME,
    publish_graph_turbo_resident_sibling, validated_graph_turbo_resident_config,
};

#[tokio::test]
async fn missing_graph_turbo_sibling_fails_before_publication() {
    let fixture = tempfile::tempdir().expect("runtime artifact fixture");
    let release_root = fixture.path().join("release");
    let protocol_home = fixture.path().join("state");
    tokio::fs::create_dir_all(&release_root)
        .await
        .expect("release root");
    let asp = release_root.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    tokio::fs::write(&asp, b"asp")
        .await
        .expect("candidate ASP artifact");

    let error = publish_graph_turbo_resident_sibling(&protocol_home, &asp)
        .await
        .expect_err("missing standalone bundle must fail");
    assert!(error.contains("standalone sibling is unavailable"));
    assert!(
        !tokio::fs::try_exists(
            protocol_home
                .join("runtime/server")
                .join(GRAPH_TURBO_CONFIG_NAME)
        )
        .await
        .expect("inspect config")
    );
}

#[tokio::test]
async fn sibling_publication_is_content_addressed_and_idempotent() {
    let fixture = tempfile::tempdir().expect("runtime artifact fixture");
    let release_root = fixture.path().join("release");
    let protocol_home = fixture.path().join("state");
    let source_bundle = release_root.join(GRAPH_TURBO_BUNDLE_NAME);
    tokio::fs::create_dir_all(source_bundle.join("_internal"))
        .await
        .expect("standalone bundle");
    let asp = release_root.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    tokio::fs::write(&asp, b"asp")
        .await
        .expect("candidate ASP artifact");
    let entry = source_bundle.join(format!(
        "{GRAPH_TURBO_EXECUTABLE_NAME}{}",
        std::env::consts::EXE_SUFFIX
    ));
    tokio::fs::write(&entry, b"resident-entry")
        .await
        .expect("resident entry");
    tokio::fs::write(source_bundle.join("_internal/runtime.dat"), b"runtime")
        .await
        .expect("resident runtime data");

    let first = publish_graph_turbo_resident_sibling(&protocol_home, &asp)
        .await
        .expect("first standalone publication");
    let second = publish_graph_turbo_resident_sibling(&protocol_home, &asp)
        .await
        .expect("idempotent standalone publication");
    assert_eq!(
        first.runtime_artifact_digest,
        second.runtime_artifact_digest
    );
    assert_eq!(first.locator, second.locator);
    assert_eq!(first.publication, "published");
    assert_eq!(second.publication, "current");
    assert_eq!(
        first.locator.parent().and_then(std::path::Path::file_name),
        Some(std::ffi::OsStr::new(GRAPH_TURBO_BUNDLE_NAME))
    );
    assert!(first.runtime_artifact_digest.starts_with("blake3-256:"));
    assert!(first.execution_artifact_digest.starts_with("blake3-256:"));

    let config: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(
            protocol_home
                .join("runtime/server")
                .join(GRAPH_TURBO_CONFIG_NAME),
        )
        .await
        .expect("resident config"),
    )
    .expect("typed resident config");
    assert_eq!(config["schemaVersion"], "1");
    assert_eq!(config["artifactKind"], "standalone-directory");
    assert_eq!(
        config["runtimeArtifactDigest"],
        first.runtime_artifact_digest
    );
    assert_eq!(
        config["executionArtifactDigest"],
        first.execution_artifact_digest
    );
    assert_eq!(
        config["executionArtifactLocator"],
        first.locator.display().to_string()
    );
}

#[tokio::test]
async fn changing_any_bundle_file_advances_the_merkle_identity() {
    let fixture = tempfile::tempdir().expect("runtime artifact fixture");
    let release_root = fixture.path().join("release");
    let protocol_home = fixture.path().join("state");
    let source_bundle = release_root.join(GRAPH_TURBO_BUNDLE_NAME);
    tokio::fs::create_dir_all(source_bundle.join("_internal"))
        .await
        .expect("standalone bundle");
    let asp = release_root.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    tokio::fs::write(&asp, b"asp")
        .await
        .expect("candidate ASP artifact");
    tokio::fs::write(
        source_bundle.join(format!(
            "{GRAPH_TURBO_EXECUTABLE_NAME}{}",
            std::env::consts::EXE_SUFFIX
        )),
        b"resident-entry",
    )
    .await
    .expect("resident entry");
    let runtime_data = source_bundle.join("_internal/runtime.dat");
    tokio::fs::write(&runtime_data, b"runtime-v1")
        .await
        .expect("runtime data");
    let first = publish_graph_turbo_resident_sibling(&protocol_home, &asp)
        .await
        .expect("first standalone publication");

    tokio::fs::write(&runtime_data, b"runtime-v2")
        .await
        .expect("changed runtime data");
    let second = publish_graph_turbo_resident_sibling(&protocol_home, &asp)
        .await
        .expect("changed standalone publication");
    assert_ne!(
        first.runtime_artifact_digest,
        second.runtime_artifact_digest
    );
    assert_ne!(first.locator, second.locator);
}

#[tokio::test]
async fn runtime_rejects_a_locator_outside_the_digest_addressed_bundle() {
    let fixture = tempfile::tempdir().expect("runtime artifact fixture");
    let release = fixture.path().join("release");
    let bundle = release.join(GRAPH_TURBO_BUNDLE_NAME);
    tokio::fs::create_dir_all(&bundle)
        .await
        .expect("bundle root");
    let asp = release.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    tokio::fs::write(&asp, b"asp").await.expect("ASP candidate");
    tokio::fs::write(
        bundle.join(format!(
            "{GRAPH_TURBO_EXECUTABLE_NAME}{}",
            std::env::consts::EXE_SUFFIX
        )),
        b"resident",
    )
    .await
    .expect("resident entry");
    publish_graph_turbo_resident_sibling(fixture.path(), &asp)
        .await
        .expect("publish bundle");
    let escaped = fixture.path().join("runtime/escaped-resident");
    tokio::fs::write(&escaped, b"resident")
        .await
        .expect("escaped entry");
    let config_path = fixture
        .path()
        .join("runtime/server")
        .join(GRAPH_TURBO_CONFIG_NAME);
    let mut config: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(&config_path)
            .await
            .expect("resident config"),
    )
    .expect("decode resident config");
    config["executionArtifactLocator"] = serde_json::json!(escaped);
    tokio::fs::write(
        &config_path,
        serde_json::to_vec_pretty(&config).expect("encode changed config"),
    )
    .await
    .expect("change resident config");

    let error = validated_graph_turbo_resident_config(fixture.path())
        .await
        .expect_err("escaped locator must fail");
    assert!(error.contains("escaped the admitted standalone bundle"));
}

#[tokio::test]
async fn runtime_rejects_entry_bytes_that_drift_after_install() {
    let fixture = tempfile::tempdir().expect("runtime artifact fixture");
    let release = fixture.path().join("release");
    let bundle = release.join(GRAPH_TURBO_BUNDLE_NAME);
    tokio::fs::create_dir_all(&bundle)
        .await
        .expect("bundle root");
    let asp = release.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    tokio::fs::write(&asp, b"asp").await.expect("ASP candidate");
    tokio::fs::write(
        bundle.join(format!(
            "{GRAPH_TURBO_EXECUTABLE_NAME}{}",
            std::env::consts::EXE_SUFFIX
        )),
        b"resident-v1",
    )
    .await
    .expect("resident entry");
    let published = publish_graph_turbo_resident_sibling(fixture.path(), &asp)
        .await
        .expect("publish bundle");
    tokio::fs::write(&published.locator, b"resident-v2")
        .await
        .expect("drift resident entry");

    let error = validated_graph_turbo_resident_config(fixture.path())
        .await
        .expect_err("entry drift must fail");
    assert!(error.contains("execution artifact digest drift"));
}

#[tokio::test]
async fn runtime_rejects_every_non_standalone_artifact_kind() {
    let fixture = tempfile::tempdir().expect("runtime artifact fixture");
    let server = fixture.path().join("runtime/server");
    tokio::fs::create_dir_all(&server)
        .await
        .expect("server root");
    tokio::fs::write(
        server.join(GRAPH_TURBO_CONFIG_NAME),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-config",
            "schemaVersion": "1",
            "artifactKind": "python-interpreter",
            "executionArtifactLocator": "/outside/python",
            "executionArtifactDigest": "blake3-256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "runtimeArtifactDigest": "blake3-256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        }))
        .expect("encode config"),
    )
    .await
    .expect("resident config");

    let error = validated_graph_turbo_resident_config(fixture.path())
        .await
        .expect_err("interpreter fallback must fail");
    assert!(error.contains("artifact kind is unsupported"));
}
