#[path = "../../../src/command/global_provider_catalog.rs"]
mod catalog;
#[allow(dead_code)]
#[path = "../../../src/command/install_provider_reconcile.rs"]
mod install_provider_reconcile;

use std::path::Path;
use std::sync::Mutex;

static ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

struct EnvironmentGuard {
    state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
}

impl EnvironmentGuard {
    fn install(state_home: &Path, home: &Path) -> Self {
        let guard = Self {
            state_home: std::env::var_os("ASP_STATE_HOME"),
            home: std::env::var_os("HOME"),
        };
        // SAFETY: the single catalog contract test owns ENVIRONMENT_LOCK and
        // restores both variables before releasing it.
        unsafe {
            std::env::set_var("ASP_STATE_HOME", state_home);
            std::env::set_var("HOME", home);
        }
        guard
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        // SAFETY: the single catalog contract test still owns ENVIRONMENT_LOCK.
        unsafe {
            match self.state_home.as_ref() {
                Some(value) => std::env::set_var("ASP_STATE_HOME", value),
                None => std::env::remove_var("ASP_STATE_HOME"),
            }
            match self.home.as_ref() {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}

fn provider(
    language_id: &str,
    materialized_path: &Path,
    artifact_digest: String,
) -> catalog::GlobalProviderCatalogProvider {
    let provider_id = agent_semantic_hook::registered_provider_id_v1(language_id)
        .unwrap_or_else(|| panic!("registered provider for {language_id}"));
    let materialized_path = materialized_path
        .canonicalize()
        .expect("canonical provider artifact")
        .to_string_lossy()
        .to_string();
    let _runtime_catalog_loader = catalog::load_runtime_provider_catalog;
    catalog::GlobalProviderCatalogProvider {
        runtime_contract: serde_json::from_value(serde_json::json!({
            "transport": "runtime-ipc",
            "operations": [{
                "operation": "project-resolution-stdin",
                "requestSchemaId": "agent.semantic-protocols.provider-project-resolution-request.v1",
                "responseSchemaId": "agent.semantic-protocols.provider-project-resolution-response.v1"
            }]
        }))
            .expect("canonical Tokio provider runtime contract"),
        language_id: language_id.to_string(),
        provider_id: provider_id.to_string(),
        manifest_id: format!("{language_id}.manifest"),
        manifest_digest: format!("blake3-256:{:064x}", language_id.len()),
        materialized_path: materialized_path.clone(),
        artifact_digest: format!("blake3-256:{artifact_digest}"),
        artifact_metadata_digest: format!(
            "blake3-256:{}",
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(Path::new(
                &materialized_path,
            ))
            .expect("provider artifact metadata digest")
        ),
        execution_command_digest: format!("sha256:{:064x}", language_id.len() + 1),
        exact_parser_identity_digest:
            agent_semantic_content_identity::exact_selector_projection_packet::
                derive_parser_identity_digest_v1(
                    &agent_semantic_content_identity::exact_selector_projection_packet::
                        ProjectionPacketProviderIdV1::from(provider_id),
                    &agent_semantic_content_identity::exact_selector_projection_packet::
                        ProjectionPacketExecutionCommandDigestV1::from(format!(
                            "sha256:{:064x}",
                            language_id.len() + 1
                        )),
                    &agent_semantic_content_identity::exact_selector_projection_packet::
                        ProjectionPacketSemanticRegistryDigestV1::from(
                            agent_semantic_hook::registered_language_descriptor_digest(language_id)
                                .expect("provider registry digest"),
                        ),
                )
                .as_str()
                .to_owned(),
        argv_prefix: vec![materialized_path],
        provider_registry_digest: agent_semantic_hook::registered_language_descriptor_digest(
            language_id,
        )
        .expect("provider registry digest"),
        query_pack_digest: agent_semantic_hook::registered_query_pack_digest(language_id)
            .expect("query pack digest"),
        exact_query_pack_identity_digest:
            agent_semantic_hook::registered_provider_catalog_identities()
                .iter()
                .find(|identity| identity.language_id == language_id)
                .expect("registered provider catalog identity")
                .exact_query_pack_identity_digest
                .clone(),
    }
}

fn write_catalog(path: &Path, providers: &[catalog::GlobalProviderCatalogProvider]) {
    let provider_bytes = serde_json::to_vec(providers).expect("encode provider generation");
    let catalog_generation = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
            &provider_bytes,
        )
        .as_str()
    );
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "asp.global-provider-catalog.v1",
            "schemaVersion": "1",
            "catalogGeneration": catalog_generation,
            "providers": providers,
        }))
        .expect("encode provider catalog"),
    )
    .expect("write provider catalog");
}

#[tokio::test]
async fn catalog_readiness_fails_closed_for_invalid_or_drifted_entries() {
    let _runtime_snapshot_entrypoint = catalog::runtime_provider_registry_snapshot;
    let _runtime_launch_entrypoint: fn(
        &catalog::RuntimeProviderCatalog,
        &std::path::Path,
        &str,
    ) -> Result<catalog::RuntimeProviderLaunch, String> =
        catalog::RuntimeProviderCatalog::runtime_launch;
    let _publication_entrypoint: fn(
        &std::path::Path,
        &[install_provider_reconcile::ProviderInstallReceipt],
    )
        -> Result<catalog::GlobalProviderCatalogPublication, String> =
        catalog::publish_global_provider_catalog;
    let _readiness_entrypoint: fn(
        &std::path::Path,
    ) -> Result<catalog::GlobalProviderCatalogReadiness, String> =
        catalog::read_global_provider_catalog_readiness;
    let _environment_lock = ENVIRONMENT_LOCK
        .lock()
        .expect("global provider catalog environment lock");
    let root = std::env::temp_dir().join(format!(
        "asp-global-provider-catalog-contract-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear catalog contract root");
    }
    let state_home = root.join("state");
    let fake_home = root.join("home");
    let runtime = state_home.join("runtime");
    std::fs::create_dir_all(&runtime).expect("create State Home runtime");
    std::fs::create_dir_all(fake_home.join(".agent-semantic-protocols/runtime"))
        .expect("create ignored HOME runtime");
    std::fs::write(
        fake_home.join(".agent-semantic-protocols/runtime/provider-catalog.v1.json"),
        b"{",
    )
    .expect("write ignored HOME catalog");
    let _environment = EnvironmentGuard::install(&state_home, &fake_home);
    let catalog_path = runtime.join("provider-catalog.v1.json");

    let missing = catalog::read_global_provider_catalog_readiness(&state_home)
        .expect_err("missing State Home catalog must fail closed");
    assert!(missing.contains("failed to read Global provider catalog"));
    std::fs::write(&catalog_path, b"{").expect("write malformed catalog");
    let malformed = catalog::read_global_provider_catalog_readiness(&state_home)
        .expect_err("malformed State Home catalog must fail closed");
    assert!(malformed.contains("failed to parse Global provider catalog"));

    let rust_path = runtime.join("rs-harness");
    let typescript_path = runtime.join("ts-harness");
    std::fs::write(&rust_path, b"rust-provider").expect("write Rust provider artifact");
    std::fs::write(&typescript_path, b"typescript-provider")
        .expect("write TypeScript provider artifact");
    let rust_digest = agent_semantic_content_identity::file_content_digest_v1(&rust_path)
        .expect("Rust provider digest");
    let typescript_digest =
        agent_semantic_content_identity::file_content_digest_v1(&typescript_path)
            .expect("TypeScript provider digest");
    let providers = vec![
        provider("rust", &rust_path, rust_digest),
        provider("typescript", &typescript_path, typescript_digest),
    ];
    write_catalog(&catalog_path, &providers);

    let readiness = catalog::read_global_provider_catalog_readiness(&state_home)
        .expect("load valid provider catalog");
    assert_eq!(readiness.provider_count, 2);

    std::fs::write(&typescript_path, b"typescript-provider-drift")
        .expect("drift TypeScript provider metadata");
    let digest_drift = catalog::read_global_provider_catalog_readiness(&state_home)
        .expect_err("artifact metadata drift must fail closed");
    assert!(digest_drift.contains("artifact"));

    let runtime_bin = runtime.join("bin");
    let provider_receipts = runtime.join("providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("create runtime provider bin");
    std::fs::create_dir_all(&provider_receipts).expect("create provider receipts");
    let mut receipts = Vec::new();
    for manifest in agent_semantic_hook::schema_registry_provider_manifests() {
        let binary_path = runtime_bin.join(manifest.binary());
        if !binary_path.exists() {
            std::fs::write(
                &binary_path,
                format!("{}-provider", manifest.binary()).as_bytes(),
            )
            .expect("write registered provider binary");
        }
        let artifact_digest = agent_semantic_content_identity::file_content_digest_v1(&binary_path)
            .expect("registered provider content digest");
        let metadata_digest =
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(&binary_path)
                .expect("registered provider metadata digest");
        let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
            &[binary_path.to_string_lossy().to_string()],
            &artifact_digest,
        )
        .expect("registered provider execution command digest");
        std::fs::write(
            provider_receipts.join(format!("{}.lock.toml", manifest.language_id())),
            format!(
                "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{}\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
                manifest.language_id(),
                manifest.provider_id(),
                binary_path.display(),
                artifact_digest,
                metadata_digest,
                execution_command_digest,
            ),
        )
        .expect("write registered provider receipt");
        receipts.push(
            install_provider_reconcile::read_provider_install_receipt(
                manifest.language_id().as_str(),
                &provider_receipts,
            )
            .expect("read registered provider receipt"),
        );
    }
    write_catalog(&catalog_path, &providers);
    let mut obsolete_catalog: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&catalog_path).expect("read obsolete provider catalog"),
    )
    .expect("decode obsolete provider catalog");
    for provider in obsolete_catalog["providers"]
        .as_array_mut()
        .expect("obsolete catalog providers")
    {
        provider
            .as_object_mut()
            .expect("obsolete catalog provider")
            .remove("exactParserIdentityDigest");
        provider
            .as_object_mut()
            .expect("obsolete catalog provider")
            .remove("exactQueryPackIdentityDigest");
    }
    std::fs::write(
        &catalog_path,
        serde_json::to_vec_pretty(&obsolete_catalog).expect("encode obsolete provider catalog"),
    )
    .expect("write obsolete provider catalog");
    let obsolete = match catalog::load_runtime_provider_catalog(&state_home).await {
        Ok(_) => panic!("strict runtime read must reject an obsolete provider catalog"),
        Err(error) => error,
    };
    assert!(obsolete.contains("exactParserIdentityDigest"));
    let changed = catalog::publish_global_provider_catalog(&state_home, &receipts)
        .expect("explicit publisher must atomically repair the obsolete provider catalog");
    assert!(changed.catalog_write);
    assert!(
        changed
            .catalog_recovery_reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("malformed-active-catalog:"))
    );
    let runtime_catalog = catalog::load_runtime_provider_catalog(&state_home)
        .await
        .expect("strict runtime read must accept the repaired provider catalog");
    let launch = runtime_catalog
        .runtime_launch(&state_home, "rust")
        .expect("repaired catalog must prepare the resident Rust provider runtime");
    assert!(launch.key.contains("rust:asp-rust:"));
    let catalog::RuntimeProviderLaunchSpec::HttpServer(spec) = launch.spec;
    assert_eq!(spec.args.last().map(String::as_str), Some("serve"));
    assert_eq!(
        spec.env.get("ASP_PROVIDER_ID").map(String::as_str),
        Some("asp-rust")
    );
    assert_eq!(
        spec.env.get("ASP_PROVIDER_LANGUAGE_ID").map(String::as_str),
        Some("rust")
    );
    assert_eq!(
        spec.env.get("ASP_PROVIDER_ARTIFACT_DIGEST"),
        Some(&launch.expected_receipt.artifact_digest)
    );
    assert_eq!(
        spec.env.get("ASP_PROVIDER_MANIFEST_DIGEST"),
        Some(&launch.expected_receipt.manifest_digest)
    );
    assert_eq!(
        spec.env.get("ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST"),
        Some(&launch.expected_receipt.contract_digest)
    );
    launch
        .expected_receipt
        .validate()
        .expect("resident provider runtime receipt");
    assert_eq!(changed.binary_byte_reads, 0);
    assert!(changed.changed_leaf_count > 0);
    assert!(changed.catalog_write);
    eprintln!(
        "[global-provider-catalog-publication] phase=cold totalMicros={} receiptMicros={} manifestMicros={} registryMicros={} queryPackMicros={}",
        changed.elapsed_micros,
        changed.receipt_read_micros,
        changed.manifest_digest_micros,
        changed.registry_digest_micros,
        changed.query_pack_digest_micros,
    );
    let catalog_document: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&catalog_path).expect("read published provider catalog"),
    )
    .expect("decode published provider catalog");
    let catalog_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/asp.global-provider-catalog.v1.schema.json"
    ))
    .expect("decode Global provider catalog schema");
    let runtime_contract_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/provider-runtime-contract-descriptor.v1.schema.json"
    ))
    .expect("decode provider runtime contract schema");
    let asp_client_server_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/asp-client-server-descriptor.v1.schema.json"
    ))
    .expect("decode ASP client-server descriptor schema");
    let registry = referencing::Registry::new()
    .add(
        "https://schemas.agent-semantic-protocols.dev/provider-runtime-contract-descriptor.v1.schema.json",
        referencing::Resource::from_contents(runtime_contract_schema),
    )
    .expect("register provider runtime contract schema")
    .add(
        "https://schemas.agent-semantic-protocols.dev/asp-client-server-descriptor.v1.schema.json",
        referencing::Resource::from_contents(asp_client_server_schema),
    )
    .expect("register ASP client-server descriptor schema")
    .prepare()
    .expect("prepare provider runtime contract schema registry");
    let catalog_validator = jsonschema::options()
        .with_registry(&registry)
        .build(&catalog_schema)
        .expect("compile Global provider catalog schema");
    if !catalog_validator.is_valid(&catalog_document) {
        let errors = catalog_validator
            .iter_errors(&catalog_document)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        panic!("published Global provider catalog violates schema: {errors:?}");
    }
    let rust_leaf = catalog_document["providers"]
        .as_array()
        .expect("catalog providers")
        .iter()
        .find(|provider| provider["languageId"] == "rust")
        .expect("Rust catalog leaf");
    assert_eq!(
        rust_leaf["exactParserIdentityDigest"]
            .as_str()
            .expect("exact parser identity")
            .len(),
        64
    );
    assert_eq!(
        rust_leaf["exactQueryPackIdentityDigest"]
            .as_str()
            .expect("exact query-pack identity")
            .len(),
        64
    );
    let manifests = agent_semantic_hook::schema_registry_provider_manifests()
        .into_iter()
        .filter(|manifest| {
            agent_semantic_hook::registered_provider_kind(manifest.language_id().as_str())
                .expect("registered provider kind")
                == agent_semantic_hook::RegisteredProviderKind::ProgrammingLanguage
        })
        .filter(|manifest| {
            receipts.iter().any(|receipt| {
                receipt.language_id == manifest.language_id().as_str()
                    || receipt
                        .installed_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        == Some(manifest.binary())
            })
        })
        .collect::<Vec<_>>();
    let current_catalog: catalog::GlobalProviderCatalog =
        serde_json::from_value(catalog_document.clone()).expect("decode current provider catalog");
    let catalog_identities = agent_semantic_hook::registered_provider_catalog_identities();
    assert!(catalog::active_catalog_matches_receipts(
        &current_catalog,
        &manifests,
        &receipts,
        catalog_identities,
    ));
    let mut stale_manifest_catalog_document = catalog_document.clone();
    stale_manifest_catalog_document["providers"][0]["manifestDigest"] =
        serde_json::Value::String(format!("sha256:{:064x}", 0));
    let stale_manifest_catalog: catalog::GlobalProviderCatalog =
        serde_json::from_value(stale_manifest_catalog_document)
            .expect("decode manifest-drift provider catalog");
    assert!(
        !catalog::active_catalog_matches_receipts(
            &stale_manifest_catalog,
            &manifests,
            &receipts,
            catalog_identities,
        ),
        "warm catalog reuse must reject manifest identity drift"
    );
    let warm = catalog::publish_global_provider_catalog(&state_home, &receipts)
        .expect("reuse unchanged receipt catalog");
    assert_eq!(warm.catalog_generation, changed.catalog_generation);
    assert_eq!(warm.binary_byte_reads, 0);
    assert_eq!(warm.changed_leaf_count, 0);
    assert!(!warm.catalog_write);
    eprintln!(
        "[global-provider-catalog-publication] phase=warm totalMicros={} receiptMicros={} manifestMicros={} registryMicros={} queryPackMicros={}",
        warm.elapsed_micros,
        warm.receipt_read_micros,
        warm.manifest_digest_micros,
        warm.registry_digest_micros,
        warm.query_pack_digest_micros,
    );
    std::fs::remove_dir_all(&runtime_bin).expect("remove provider bytes after catalog publication");

    std::fs::remove_dir_all(root).expect("remove catalog contract root");
}

#[tokio::test]
async fn clean_state_home_admits_empty_runtime_catalog_without_weakening_strict_reads() {
    let _environment_lock = ENVIRONMENT_LOCK
        .lock()
        .expect("global provider catalog environment lock");
    let root = std::env::temp_dir().join(format!(
        "asp-empty-runtime-provider-catalog-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear empty runtime catalog root");
    }
    let state_home = root.join("state");
    let fake_home = root.join("home");
    std::fs::create_dir_all(state_home.join("runtime")).expect("create clean State Home runtime");
    std::fs::create_dir_all(&fake_home).expect("create isolated HOME");
    let _environment = EnvironmentGuard::install(&state_home, &fake_home);

    let readiness = catalog::read_runtime_provider_catalog_readiness(&state_home)
        .expect("clean State Home must admit an empty runtime provider catalog");
    assert_eq!(readiness.provider_count, 0);
    assert!(readiness.catalog_generation.starts_with("blake3-256:"));

    let runtime_catalog = catalog::load_runtime_provider_catalog(&state_home)
        .await
        .expect("Runtime daemon must consume the canonical empty catalog");
    let no_provider = match runtime_catalog.runtime_launch(&state_home, "rust") {
        Ok(_) => panic!("empty catalog must not dispatch a provider"),
        Err(error) => error,
    };
    assert!(no_provider.contains("Runtime search provider is not registered"));

    let strict = catalog::read_global_provider_catalog_readiness(&state_home)
        .expect_err("provider dispatch must still reject a missing catalog");
    assert!(strict.contains("failed to read Global provider catalog"));

    std::fs::remove_dir_all(root).expect("cleanup empty runtime catalog root");
}
