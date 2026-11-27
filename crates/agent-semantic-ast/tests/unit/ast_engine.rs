// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! End-to-end AI-Native Language Server Runtime tests for `agent-semantic-ast`
//! (RFC 10.35): Provider Registry routing, `inspect_file` Open EDB streaming for
//! Scheme `:gerbil-ascent`, lossless span splicing, Frontier 2 calibration, and
//! Resident Actor transport integration.

mod support;

use std::fs;
use std::sync::Arc;

use agent_semantic_ast::{
    AST_INSPECT_OPERATION, AST_VERIFY_OPERATION, AstCalibrationOutcome, AstEngine, AstEngineError,
    AstInspectRequest, AstInspectResponse, AstLanguageId, AstNodeLocator, AstPatchOp,
    AstPatchRequest, AstPatchStatus, AstProviderCapabilities, AstProviderClientConfig,
    AstProviderDescriptor, AstProviderDocumentBinding, AstProviderId, AstProviderPort,
    AstProviderRegistry, AstProviderTransportKind, AstSyntaxDiagnostic, AstVerifyCandidateResult,
    AstVerifyRequest, AstVerifyResponse, AstWorkspacePath, HttpJsonAstProviderClient,
    LexicalDelimiterProfile, ResidentActorAstProvider, blake3_source_digest,
};
use agent_semantic_provider_protocol::ProviderSchemaReference;
use agent_semantic_provider_transport::{
    ProviderRuntimeContractOperation, ProviderRuntimeContractReceipt,
    ProviderRuntimeContractTransport, spawn_in_process_provider_runtime_actor,
};
use agent_semantic_search::WorkspaceSearchIndex;
use bytes::Bytes;
use tempfile::TempDir;

use support::{
    braced_http_dispatch, canonical_provider_document, inspect_braced_source, sexp_http_dispatch,
    spawn_http_provider_server,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_register_engine_routes_multi_language_h1_h2_over_http_json() {
    let (br_url, br_shutdown, br_handle) = spawn_http_provider_server(braced_http_dispatch).await;
    let (ss_url, ss_shutdown, ss_handle) = spawn_http_provider_server(sexp_http_dispatch).await;

    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join("src")).expect("mkdir src");
    let br_src = "// License header — MUST stay bit-for-bit identical\n\
struct SessionId { raw: str }\n\
\n\
// Untouched sibling comment\n\
fn helper() { return 1 }\n\
\n\
fn target(x) {\n\
    return x + 1\n\
}\n";
    let ss_src = "; Scheme comment with unbalanced ))) ignored by profile\n\
(def (keep-me x) (+ x 1))\n\
(def (target-fn x) (+ x 2))\n";
    fs::write(tmp.path().join("src/app.br"), br_src).expect("write app.br");
    fs::write(tmp.path().join("src/rules.ss"), ss_src).expect("write rules.ss");
    fs::write(tmp.path().join("src/unknown.py"), "def f(): pass\n").expect("write unknown.py");

    let mut engine = AstEngine::new(tmp.path().to_path_buf());
    let caps = AstProviderCapabilities::default();
    let r1 = engine
        .providers_mut()
        .register_from_provider_document(
            &canonical_provider_document("braced-demo", "asp-braced-demo", &[".br"]),
            AstProviderDocumentBinding {
                lexical_profile: LexicalDelimiterProfile::rust(),
                capabilities: caps,
                transport_kind: AstProviderTransportKind::HttpJsonLoopback,
                endpoint: Some(br_url.clone()),
            },
            Arc::new(HttpJsonAstProviderClient::new(
                br_url,
                AstProviderClientConfig {
                    provider_id: AstProviderId::new("asp-braced-demo").expect("pid"),
                    language_id: AstLanguageId::new("braced-demo").expect("lid"),
                    lexical_profile: LexicalDelimiterProfile::rust(),
                    capabilities: caps,
                },
            )),
        )
        .expect("register braced provider");
    assert_eq!(r1.generation, 1);

    let r2 = engine
        .providers_mut()
        .register_from_provider_document(
            &canonical_provider_document("sexp-demo", "asp-sexp-demo", &[".ss"]),
            AstProviderDocumentBinding {
                lexical_profile: LexicalDelimiterProfile::scheme(),
                capabilities: caps,
                transport_kind: AstProviderTransportKind::HttpJsonLoopback,
                endpoint: Some(ss_url.clone()),
            },
            Arc::new(HttpJsonAstProviderClient::new(
                ss_url,
                AstProviderClientConfig {
                    provider_id: AstProviderId::new("asp-sexp-demo").expect("pid"),
                    language_id: AstLanguageId::new("sexp-demo").expect("lid"),
                    lexical_profile: LexicalDelimiterProfile::scheme(),
                    capabilities: caps,
                },
            )),
        )
        .expect("register sexp provider");
    assert_eq!(r2.generation, 2);
    assert_ne!(r1.registry_digest, r2.registry_digest);

    // `inspect_file` streams native AST spans & Open EDB facts for Scheme `:gerbil-ascent`.
    let inspected = engine
        .inspect_file(&AstWorkspacePath::new("src/app.br").expect("path"), None)
        .expect("inspect_file");
    assert!(inspected.accepted);
    assert_eq!(inspected.open_edb_batch.entities.len(), 3);
    assert!(
        inspected
            .open_edb_batch
            .entities
            .iter()
            .any(|ent| ent.name == "SessionId" && ent.kind == "struct")
    );

    // Unregistered `.py` fails closed.
    let err = engine
        .apply_patch(&AstPatchRequest {
            path: AstWorkspacePath::new("src/unknown.py").expect("path"),
            language_id: None,
            expected_preimage_hash: None,
            operation: AstPatchOp::ApplySpanEdits { edits: Vec::new() },
            dry_run: true,
            auto_calibrate: true,
        })
        .expect_err("unregistered extension must fail closed");
    assert!(matches!(err, AstEngineError::ProviderNotRegistered(_)));

    // `.br` routes to `asp-braced-demo`, calibrates missing `}`, preserves trivia bit-for-bit.
    let br_receipt = engine
        .apply_patch(&AstPatchRequest {
            path: AstWorkspacePath::new("src/app.br").expect("path"),
            language_id: None,
            expected_preimage_hash: Some(blake3_source_digest(br_src.as_bytes())),
            operation: AstPatchOp::ReplaceItem {
                target: AstNodeLocator::new("src/app.br#fn:target").expect("loc"),
                snippet: "fn target(x) {\n    return x * 10\n".to_owned(),
            },
            dry_run: false,
            auto_calibrate: true,
        })
        .expect("apply braced patch");
    assert_eq!(br_receipt.status, AstPatchStatus::CalibratedAndApplied);
    assert_eq!(br_receipt.provider_id.as_str(), "asp-braced-demo");
    let updated_br = fs::read_to_string(tmp.path().join("src/app.br")).expect("read br");
    assert!(updated_br.contains("// License header — MUST stay bit-for-bit identical"));
    assert!(updated_br.contains("// Untouched sibling comment\nfn helper() { return 1 }"));
    assert!(updated_br.contains("return x * 10") && updated_br.trim_end().ends_with('}'));

    // `.ss` routes to `asp-sexp-demo`, uses Scheme comment profile, calibrates missing `)`.
    let ss_receipt = engine
        .apply_patch(&AstPatchRequest {
            path: AstWorkspacePath::new("src/rules.ss").expect("path"),
            language_id: None,
            expected_preimage_hash: None,
            operation: AstPatchOp::ReplaceItem {
                target: AstNodeLocator::new("src/rules.ss#def:target-fn").expect("loc"),
                snippet: "(def (target-fn x) (* x 3)".to_owned(),
            },
            dry_run: false,
            auto_calibrate: true,
        })
        .expect("apply sexp patch");
    assert_eq!(ss_receipt.status, AstPatchStatus::CalibratedAndApplied);
    assert_eq!(ss_receipt.provider_id.as_str(), "asp-sexp-demo");
    let updated_ss = fs::read_to_string(tmp.path().join("src/rules.ss")).expect("read ss");
    assert!(updated_ss.contains("; Scheme comment with unbalanced ))) ignored by profile"));
    assert!(updated_ss.contains("(def (target-fn x) (* x 3))"));

    // Scheme `:gerbil-ascent` style policy executes its computed span rewrite cleanly
    // through the pure Rust Runtime via `AstPatchOp::ApplySpanEdits`.
    let target_node = engine
        .inspect_file(&AstWorkspacePath::new("src/app.br").expect("path"), None)
        .expect("re-inspect")
        .nodes
        .into_iter()
        .find(|n| n.name == "helper")
        .expect("helper node");
    let policy_patch = engine
        .apply_patch(&AstPatchRequest {
            path: AstWorkspacePath::new("src/app.br").expect("path"),
            language_id: None,
            expected_preimage_hash: None,
            operation: AstPatchOp::ApplySpanEdits {
                edits: vec![(
                    target_node.inner_span,
                    "fn helper() { return 99 }".to_owned(),
                )],
            },
            dry_run: false,
            auto_calibrate: true,
        })
        .expect("apply Scheme-computed span edit");
    assert_eq!(policy_patch.status, AstPatchStatus::Applied);
    assert!(
        fs::read_to_string(tmp.path().join("src/app.br"))
            .expect("read")
            .contains("fn helper() { return 99 }")
    );

    let r3 = engine
        .providers_mut()
        .unregister(&AstProviderId::new("asp-sexp-demo").expect("pid"))
        .expect("unregister");
    assert_eq!(r3.generation, 3);
    assert!(
        engine
            .providers()
            .resolve_for_path(&AstWorkspacePath::new("src/rules.ss").expect("path"))
            .is_err()
    );

    let _ = br_shutdown.send(true);
    let _ = ss_shutdown.send(true);
    br_handle.abort();
    ss_handle.abort();
}

#[test]
fn h2_fail_closed_on_ambiguous_tie_never_mutates_disk() {
    struct AmbiguousOracleProvider {
        pid: AstProviderId,
        lid: AstLanguageId,
    }
    impl AstProviderPort for AmbiguousOracleProvider {
        fn provider_id(&self) -> &AstProviderId {
            &self.pid
        }
        fn language_id(&self) -> &AstLanguageId {
            &self.lid
        }
        fn lexical_profile(&self) -> LexicalDelimiterProfile {
            LexicalDelimiterProfile::rust()
        }
        fn inspect(&self, req: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError> {
            Ok(inspect_braced_source(&req.path, &req.source_utf8))
        }
        fn verify(&self, req: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
            let results = req
                .candidates
                .iter()
                .map(|cand| {
                    let accepted = cand.candidate_id != "raw";
                    AstVerifyCandidateResult {
                        candidate_id: cand.candidate_id.clone(),
                        accepted,
                        diagnostics: if accepted {
                            Vec::new()
                        } else {
                            vec![AstSyntaxDiagnostic {
                                byte_start: 10,
                                byte_end: 12,
                                message: "unclosed brace".to_owned(),
                                expected_tokens: vec!["}".to_owned()],
                            }]
                        },
                    }
                })
                .collect();
            Ok(AstVerifyResponse { results })
        }
    }

    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join("src")).expect("mkdir src");
    let orig = "fn a() { return 1 }\nfn b() { return 2 }\n";
    fs::write(tmp.path().join("src/tie.br"), orig).expect("write tie.br");

    let mut registry = AstProviderRegistry::new();
    let pid = AstProviderId::new("asp-ambig").expect("pid");
    let lid = AstLanguageId::new("braced-demo").expect("lid");
    registry
        .register(
            AstProviderDescriptor {
                provider_id: pid.clone(),
                language_id: lid.clone(),
                source_extensions: vec![".br".to_owned()],
                lexical_profile: LexicalDelimiterProfile::rust(),
                capabilities: AstProviderCapabilities::default(),
                transport_kind: AstProviderTransportKind::EmbeddedPort,
                endpoint: None,
            },
            Arc::new(AmbiguousOracleProvider { pid, lid }),
        )
        .expect("register ambiguous provider");
    let engine = AstEngine::with_registry(tmp.path().to_path_buf(), registry);

    let receipt = engine
        .apply_patch(&AstPatchRequest {
            path: AstWorkspacePath::new("src/tie.br").expect("path"),
            language_id: None,
            expected_preimage_hash: None,
            operation: AstPatchOp::ReplaceItem {
                target: AstNodeLocator::new("src/tie.br#fn:a").expect("loc"),
                snippet: "fn a() { return 9".to_owned(),
            },
            dry_run: false,
            auto_calibrate: true,
        })
        .expect("ambiguous tie returns receipt");
    assert_eq!(receipt.status, AstPatchStatus::RejectedFailClosed);
    assert_eq!(
        receipt.calibration.as_ref().expect("cal").outcome,
        AstCalibrationOutcome::RejectedAmbiguousTie
    );
    assert!(receipt.typed_hole.expect("hole").ambiguous_candidates >= 2);
    assert_eq!(
        fs::read_to_string(tmp.path().join("src/tie.br")).expect("disk unchanged"),
        orig
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resident_actor_provider_integrates_with_registry_and_open_edb() {
    let digest = format!("blake3-256:{}", blake3::hash(b"asp-demo-actor").to_hex());
    let contract = ProviderRuntimeContractReceipt::new(
        "asp-actor-demo",
        "braced-demo",
        digest.clone(),
        digest,
        ProviderRuntimeContractTransport::InProcess,
        [AST_INSPECT_OPERATION, AST_VERIFY_OPERATION]
            .into_iter()
            .map(|op| ProviderRuntimeContractOperation {
                operation: op.to_owned(),
                request_schema: ProviderSchemaReference {
                    schema_id: format!("agent.semantic-protocols.{op}-request"),
                    schema_version: "1".to_owned(),
                },
                response_schema: ProviderSchemaReference {
                    schema_id: format!("agent.semantic-protocols.{op}-response"),
                    schema_version: "1".to_owned(),
                },
            })
            .collect(),
    )
    .expect("valid contract receipt");

    let authority = spawn_in_process_provider_runtime_actor(
        8,
        move || async move { Ok(contract) },
        |operation, payload| async move {
            let value: serde_json::Value =
                serde_json::from_slice(&payload).map_err(|e| e.to_string())?;
            let out = braced_http_dispatch(&operation, value)?;
            serde_json::to_vec(&out)
                .map(Bytes::from)
                .map_err(|e| e.to_string())
        },
    );
    let mut client = authority.client();
    client.wait_ready().await.expect("actor ready");

    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join("src")).expect("mkdir src");
    fs::write(
        tmp.path().join("src/live.br"),
        "fn old_name() { return 1 }\n",
    )
    .expect("write");

    let mut engine = AstEngine::new(tmp.path().to_path_buf());
    let pid = AstProviderId::new("asp-actor-demo").expect("pid");
    let lid = AstLanguageId::new("braced-demo").expect("lid");
    engine
        .providers_mut()
        .register(
            AstProviderDescriptor {
                provider_id: pid.clone(),
                language_id: lid.clone(),
                source_extensions: vec![".br".to_owned()],
                lexical_profile: LexicalDelimiterProfile::rust(),
                capabilities: AstProviderCapabilities::default(),
                transport_kind: AstProviderTransportKind::ResidentActor,
                endpoint: None,
            },
            Arc::new(ResidentActorAstProvider::new(
                client,
                AstProviderClientConfig {
                    provider_id: pid,
                    language_id: lid,
                    lexical_profile: LexicalDelimiterProfile::rust(),
                    capabilities: AstProviderCapabilities::default(),
                },
            )),
        )
        .expect("register resident actor provider");

    let mut index = WorkspaceSearchIndex::build(tmp.path());
    let receipt = engine
        .apply_patch_with_index(
            &AstPatchRequest {
                path: AstWorkspacePath::new("src/live.br").expect("path"),
                language_id: None,
                expected_preimage_hash: None,
                operation: AstPatchOp::ReplaceItem {
                    target: AstNodeLocator::new("src/live.br#fn:old_name").expect("loc"),
                    snippet: "fn refreshed_symbol() { return 42 }".to_owned(),
                },
                dry_run: false,
                auto_calibrate: true,
            },
            &mut index,
        )
        .expect("apply patch with index");
    assert_eq!(receipt.status, AstPatchStatus::Applied);
    assert_eq!(receipt.open_edb_ingested_entities, 1);
    let edb = index.project_prefiltered_edb(&Default::default());
    assert!(
        edb.entities
            .iter()
            .any(|ent| ent.name == "refreshed_symbol" && ent.kind == "function")
    );

    let _ = authority.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_multi_provider_inspection_runs_providers_in_parallel_with_fault_isolation() {
    use agent_semantic_ast::{
        AspForwardingGateway, AstInspectResponse, AstNodeSpanRecord, AstUniversalKind, ByteSpan,
        ForwardedInspectFileInput, ForwardedInspectFilesRequest,
    };
    use agent_semantic_provider_protocol::{
        PROVIDER_CORE_MANIFEST_SCHEMA_ID, PROVIDER_CORE_MANIFEST_SCHEMA_VERSION,
        ProviderCapabilities, ProviderCoreManifest, ProviderLanguageId, ProviderPackageId,
        ProviderSyntaxSurface, ProviderTransport,
    };
    use std::time::{Duration, Instant};

    struct DelayedPort {
        pid: AstProviderId,
        lid: AstLanguageId,
        delay: Duration,
        fail: bool,
    }
    impl AstProviderPort for DelayedPort {
        fn provider_id(&self) -> &AstProviderId {
            &self.pid
        }
        fn language_id(&self) -> &AstLanguageId {
            &self.lid
        }
        fn lexical_profile(&self) -> LexicalDelimiterProfile {
            LexicalDelimiterProfile::rust()
        }
        fn inspect(&self, req: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError> {
            std::thread::sleep(self.delay);
            if self.fail {
                return Err(AstEngineError::ProviderError(format!(
                    "simulated provider crash on {}",
                    req.path.as_str()
                )));
            }
            Ok(AstInspectResponse {
                accepted: true,
                nodes: vec![AstNodeSpanRecord {
                    locator: AstNodeLocator::new(format!("{}#fn:demo", req.path.as_str()))
                        .expect("loc"),
                    kind: AstUniversalKind::Function,
                    language_kind: agent_semantic_ast::AstLanguageKind::new("fn_decl").expect("lk"),
                    name: format!("fn_{}", self.lid.as_str()),
                    line: 1,
                    outer_span: ByteSpan::new(0, req.source_utf8.len().max(1)).expect("span"),
                    inner_span: ByteSpan::new(0, req.source_utf8.len().max(1)).expect("span"),
                    body_span: None,
                    parent: None,
                    attrs: Vec::new(),
                }],
                edges: Vec::new(),
                diagnostics: Vec::new(),
                syntax_surface: None,
            })
        }
        fn verify(&self, _req: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
            Ok(AstVerifyResponse {
                results: Vec::new(),
            })
        }
    }

    fn mk_manifest(lang: &str, ext: &str) -> ProviderCoreManifest {
        ProviderCoreManifest {
            schema_id: PROVIDER_CORE_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: PROVIDER_CORE_MANIFEST_SCHEMA_VERSION.to_owned(),
            language_id: ProviderLanguageId::new(lang).expect("lang"),
            provider_id: ProviderPackageId::new(format!("asp-{lang}")).expect("pid"),
            display_name: lang.to_owned(),
            extensions: vec![ext.to_owned()],
            project_markers: Vec::new(),
            project: None,
            syntax_surface: ProviderSyntaxSurface {
                line_comment: Some("//".to_owned()),
                block_comment: None,
                delimiter_pairs: vec![["(".to_owned(), ")".to_owned()]],
                statement_terminator: Some(";".to_owned()),
                string_delimiters: vec!["\"".to_owned()],
            },
            transport: ProviderTransport {
                transport: "http-json".to_owned(),
                command: vec!["serve".to_owned()],
                health_path: "/health".to_owned(),
                request_path: "/v1/provider-runtime".to_owned(),
                shutdown_path: "/shutdown".to_owned(),
            },
            capabilities: ProviderCapabilities {
                ast_inspect: true,
                ast_verify: true,
                open_edb: true,
                project_resolution: false,
            },
            special: None,
        }
    }

    let gateway = Arc::new(AspForwardingGateway::new());
    for (lang, ext, fail) in [
        ("langa", ".la", false),
        ("langb", ".lb", false),
        ("langc", ".lc", false),
        ("broken", ".brk", true),
    ] {
        gateway
            .register_embedded_provider(
                mk_manifest(lang, ext),
                Arc::new(DelayedPort {
                    pid: AstProviderId::new(format!("asp-{lang}")).expect("pid"),
                    lid: AstLanguageId::new(lang).expect("lid"),
                    delay: Duration::from_millis(80),
                    fail,
                }),
                None,
                None,
            )
            .expect("register embedded provider");
    }

    let started = Instant::now();
    let resp = gateway
        .inspect_files_to_open_edb(ForwardedInspectFilesRequest {
            files: vec![
                ForwardedInspectFileInput {
                    path: "src/one.la".to_owned(),
                    source_utf8: "fn a() {}".to_owned(),
                    language_override: None,
                },
                ForwardedInspectFileInput {
                    path: "src/crash.brk".to_owned(),
                    source_utf8: "boom".to_owned(),
                    language_override: None,
                },
                ForwardedInspectFileInput {
                    path: "src/two.lb".to_owned(),
                    source_utf8: "fn b() {}".to_owned(),
                    language_override: None,
                },
                ForwardedInspectFileInput {
                    path: "src/ignored.xyz".to_owned(),
                    source_utf8: "noop".to_owned(),
                    language_override: None,
                },
                ForwardedInspectFileInput {
                    path: "src/three.lc".to_owned(),
                    source_utf8: "fn c() {}".to_owned(),
                    language_override: None,
                },
            ],
        })
        .await
        .expect("concurrent gateway inspection");
    let wall = started.elapsed();

    // 4 x 80ms = 320ms serial; parallel multi-provider dispatch must stay well below 240ms.
    assert!(
        wall < Duration::from_millis(240),
        "expected concurrent multi-provider speedup (<240ms), got {wall:?}"
    );
    assert_eq!(resp.inspected_files, 3);
    assert_eq!(resp.total_entities, 3);
    assert_eq!(resp.skipped_paths, vec!["src/ignored.xyz"]);
    assert_eq!(resp.failed_paths.len(), 1);
    assert_eq!(resp.failed_paths[0].path, "src/crash.brk");
    assert_eq!(resp.failed_paths[0].language_id, "broken");
    // Deterministic input ordering preserved despite parallel completion!
    let batch_langs: Vec<Option<&str>> = resp
        .batches
        .iter()
        .map(|b| b.language_id.as_deref())
        .collect();
    assert_eq!(
        batch_langs,
        vec![Some("langa"), Some("langb"), Some("langc")]
    );
}
