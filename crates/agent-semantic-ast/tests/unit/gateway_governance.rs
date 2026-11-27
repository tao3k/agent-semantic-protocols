// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! P2 Provider Engineering Governance unit & loopback integration tests:
//! - Per-language circuit breaker opening, fast-fail isolation, and half-open recovery
//! - Loopback Bearer control-plane authentication (`401` rejected vs `200` admitted)
//! - Structured `/health` telemetry and `perLanguageMicros` receipts

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use agent_semantic_ast::{
    AspForwardingGateway, AstEngineError, AstInspectRequest, AstInspectResponse, AstLanguageId,
    AstLanguageKind, AstNodeLocator, AstNodeSpanRecord, AstProviderId, AstProviderPort,
    AstUniversalKind, AstVerifyRequest, AstVerifyResponse, ByteSpan, CircuitBreakerState,
    ForwardedInspectFileInput, ForwardedInspectFilesRequest, GatewaySessionToken,
    LexicalDelimiterProfile, ProviderCircuitBreakerPolicy,
};
use agent_semantic_provider_protocol::{
    PROVIDER_CORE_MANIFEST_SCHEMA_ID, PROVIDER_CORE_MANIFEST_SCHEMA_VERSION, ProviderCapabilities,
    ProviderCoreManifest, ProviderLanguageId, ProviderPackageId, ProviderSyntaxSurface,
    ProviderTransport,
};
use tokio::net::TcpListener;
use tokio::sync::watch;

struct FlakyTestProvider {
    pid: AstProviderId,
    lid: AstLanguageId,
    fail_mode: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
}

impl AstProviderPort for FlakyTestProvider {
    fn provider_id(&self) -> &AstProviderId {
        &self.pid
    }

    fn language_id(&self) -> &AstLanguageId {
        &self.lid
    }

    fn lexical_profile(&self) -> LexicalDelimiterProfile {
        LexicalDelimiterProfile::rust()
    }

    fn inspect(&self, request: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_mode.load(Ordering::SeqCst) {
            return Err(AstEngineError::ProviderError(
                "simulated compiler crash".to_owned(),
            ));
        }
        Ok(AstInspectResponse {
            accepted: true,
            nodes: vec![AstNodeSpanRecord {
                locator: AstNodeLocator::new(format!("{}#fn:ok", request.path.as_str()))
                    .expect("locator"),
                kind: AstUniversalKind::Function,
                language_kind: AstLanguageKind::new("fn_decl").expect("lk"),
                name: "ok".to_owned(),
                line: 1,
                outer_span: ByteSpan::new(0, request.source_utf8.len()).expect("span"),
                inner_span: ByteSpan::new(0, request.source_utf8.len()).expect("span"),
                body_span: None,
                parent: None,
                attrs: Vec::new(),
            }],
            edges: Vec::new(),
            diagnostics: Vec::new(),
            syntax_surface: None,
        })
    }

    fn verify(&self, _request: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
        Ok(AstVerifyResponse {
            results: Vec::new(),
        })
    }
}

fn demo_manifest(lang: &str, ext: &str) -> ProviderCoreManifest {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn per_language_circuit_breaker_opens_isolates_siblings_and_recovers() {
    let policy = ProviderCircuitBreakerPolicy {
        failure_threshold: 2,
        cooldown: Duration::from_millis(80),
    };
    let gateway = Arc::new(AspForwardingGateway::with_governance(policy, None));

    let flaky_fail = Arc::new(AtomicBool::new(true));
    let flaky_calls = Arc::new(AtomicUsize::new(0));
    let healthy_calls = Arc::new(AtomicUsize::new(0));

    gateway
        .register_embedded_provider(
            demo_manifest("flaky", ".flk"),
            Arc::new(FlakyTestProvider {
                pid: AstProviderId::new("asp-flaky").expect("pid"),
                lid: AstLanguageId::new("flaky").expect("lid"),
                fail_mode: Arc::clone(&flaky_fail),
                calls: Arc::clone(&flaky_calls),
            }),
            None,
            None,
        )
        .expect("register flaky");
    gateway
        .register_embedded_provider(
            demo_manifest("healthy", ".hlth"),
            Arc::new(FlakyTestProvider {
                pid: AstProviderId::new("asp-healthy").expect("pid"),
                lid: AstLanguageId::new("healthy").expect("lid"),
                fail_mode: Arc::new(AtomicBool::new(false)),
                calls: Arc::clone(&healthy_calls),
            }),
            None,
            None,
        )
        .expect("register healthy");

    // First two calls trip the circuit breaker for `flaky` while `healthy` keeps working.
    for idx in 0..2 {
        let resp = gateway
            .inspect_files_to_open_edb(ForwardedInspectFilesRequest {
                files: vec![
                    ForwardedInspectFileInput {
                        path: format!("src/a_{idx}.flk"),
                        source_utf8: "fn x() {}".to_owned(),
                        language_override: None,
                    },
                    ForwardedInspectFileInput {
                        path: format!("src/b_{idx}.hlth"),
                        source_utf8: "fn y() {}".to_owned(),
                        language_override: None,
                    },
                ],
            })
            .await
            .expect("inspect batch");
        assert_eq!(resp.inspected_files, 1);
        assert_eq!(resp.failed_paths.len(), 1);
        assert!(resp.per_language_micros.contains_key("healthy"));
    }
    assert_eq!(flaky_calls.load(Ordering::SeqCst), 2);

    // Third call must short-circuit BEFORE touching `flaky` provider!
    let open_resp = gateway
        .inspect_files_to_open_edb(ForwardedInspectFilesRequest {
            files: vec![
                ForwardedInspectFileInput {
                    path: "src/a_open.flk".to_owned(),
                    source_utf8: "fn x() {}".to_owned(),
                    language_override: None,
                },
                ForwardedInspectFileInput {
                    path: "src/b_open.hlth".to_owned(),
                    source_utf8: "fn y() {}".to_owned(),
                    language_override: None,
                },
            ],
        })
        .await
        .expect("inspect during open circuit");
    assert_eq!(flaky_calls.load(Ordering::SeqCst), 2);
    assert_eq!(open_resp.inspected_files, 1);
    assert_eq!(open_resp.failed_paths.len(), 1);
    assert!(
        open_resp.failed_paths[0]
            .error
            .contains("circuit breaker open")
    );

    let health = gateway.health_receipts().expect("health");
    let flaky_health = health
        .iter()
        .find(|h| h.language_id == "flaky")
        .expect("flaky health");
    assert_eq!(flaky_health.circuit_state, CircuitBreakerState::Open);
    assert_eq!(flaky_health.consecutive_failures, 2);

    // Recover provider and wait past cooldown -> half-open probe succeeds and closes circuit!
    flaky_fail.store(false, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let recovered = gateway
        .inspect_files_to_open_edb(ForwardedInspectFilesRequest {
            files: vec![ForwardedInspectFileInput {
                path: "src/a_recovered.flk".to_owned(),
                source_utf8: "fn ok() {}".to_owned(),
                language_override: None,
            }],
        })
        .await
        .expect("recovered inspect");
    assert_eq!(recovered.inspected_files, 1);
    assert!(recovered.failed_paths.is_empty());
    assert_eq!(flaky_calls.load(Ordering::SeqCst), 3);

    let health_after = gateway.health_receipts().expect("health after recovery");
    let flaky_after = health_after
        .iter()
        .find(|h| h.language_id == "flaky")
        .expect("flaky health after");
    assert_eq!(flaky_after.circuit_state, CircuitBreakerState::Closed);
    assert_eq!(flaky_after.consecutive_failures, 0);
}

fn raw_http_call(
    addr: std::net::SocketAddr,
    method: &str,
    path: &str,
    auth: Option<&str>,
    body: &str,
) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect gateway");
    let auth_line = match auth {
        Some(val) => format!("Authorization: {val}\r\n"),
        None => String::new(),
    };
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\n{auth_line}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).expect("write request");
    stream.flush().expect("flush");
    let mut resp = String::new();
    stream.read_to_string(&mut resp).expect("read response");
    let status = resp
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .expect("HTTP status");
    let resp_body = resp.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    (status, resp_body)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_plane_routes_enforce_loopback_bearer_token() {
    let token = GatewaySessionToken::generate();
    let gateway = Arc::new(AspForwardingGateway::with_governance(
        ProviderCircuitBreakerPolicy::default(),
        Some(token.clone()),
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (tx, rx) = watch::channel(false);
    let srv_gw = Arc::clone(&gateway);
    let handle = tokio::spawn(async move { srv_gw.serve(listener, tx, rx).await });

    // GET /health stays open and advertises controlAuthRequired=true.
    let (health_status, health_body) =
        tokio::task::spawn_blocking(move || raw_http_call(addr, "GET", "/health", None, ""))
            .await
            .expect("join health");
    assert_eq!(health_status, 200);
    let health_json: serde_json::Value = serde_json::from_str(&health_body).expect("json");
    assert_eq!(health_json["controlAuthRequired"], true);

    // POST /v1/providers/register without token -> 401 Unauthorized.
    let (unauth_reg, _) = tokio::task::spawn_blocking(move || {
        raw_http_call(addr, "POST", "/v1/providers/register", None, "{}")
    })
    .await
    .expect("join unauth reg");
    assert_eq!(unauth_reg, 401);

    // POST /shutdown with wrong token -> 401 Unauthorized.
    let (bad_shutdown, _) = tokio::task::spawn_blocking(move || {
        raw_http_call(
            addr,
            "POST",
            "/shutdown",
            Some("Bearer wrong-token-value-123456"),
            "{}",
        )
    })
    .await
    .expect("join bad shutdown");
    assert_eq!(bad_shutdown, 401);

    // POST /shutdown with valid Bearer token -> 200 OK & clean server exit!
    let bearer = format!("Bearer {}", token.as_str());
    let (ok_shutdown, _) = tokio::task::spawn_blocking(move || {
        raw_http_call(addr, "POST", "/shutdown", Some(&bearer), "{}")
    })
    .await
    .expect("join ok shutdown");
    assert_eq!(ok_shutdown, 200);

    handle
        .await
        .expect("server task join")
        .expect("clean server shutdown");
}

#[test]
fn asp_gateway_lazy_boot_idle_eviction_and_stdin_eof_watchdog() {
    let asp_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ASP repo root");
    let gateway_bin = Path::new(env!("CARGO_BIN_EXE_asp-gateway"));
    if !asp_root
        .join("languages/asp-rust/target/debug/asp-rust")
        .exists()
        && !asp_root
            .join("languages/asp-rust/target/release/asp-rust")
            .exists()
    {
        return;
    }

    let mut child = Command::new(gateway_bin)
        .args(["--asp-root"])
        .arg(asp_root)
        .args(["--idle-timeout-secs", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn asp-gateway binary");

    let stdin = child.stdin.take().expect("gateway stdin");
    let stdout = child.stdout.take().expect("gateway stdout");
    let mut reader = BufReader::new(stdout);
    let mut boot_line = String::new();
    let n = reader.read_line(&mut boot_line).expect("read bootstrap");
    assert!(n > 0, "asp-gateway exited prematurely");

    let boot: serde_json::Value = serde_json::from_str(boot_line.trim()).expect("bootstrap JSON");
    assert_eq!(boot["spawnMode"], "lazy-on-demand");
    let endpoint = boot["endpoint"].as_str().expect("endpoint");
    let authority = endpoint.strip_prefix("http://").expect("http://");
    let addr: std::net::SocketAddr = authority.parse().expect("socket addr");

    // Immediately after startup, ZERO child compilers are spawned (all dormant).
    let (h0_status, h0_body) = raw_http_call(addr, "GET", "/health", None, "");
    assert_eq!(h0_status, 200);
    let h0: serde_json::Value = serde_json::from_str(&h0_body).expect("health JSON");
    assert_eq!(h0["activeRuntimeCount"], 0);
    assert_eq!(h0["controlAuthRequired"], true);

    // Inspect ONLY a Rust file -> ONLY `rust` boots (Julia/Python/TS/Scheme stay dormant!).
    let inspect_body = serde_json::json!({
        "files": [{
            "path": "src/demo.rs",
            "sourceUtf8": "pub fn lazy_probe() -> usize { 42 }\n"
        }]
    })
    .to_string();
    let (insp_status, insp_body) = raw_http_call(
        addr,
        "POST",
        "/v1/open-edb/inspect-files",
        None,
        &inspect_body,
    );
    assert_eq!(insp_status, 200);
    let insp: serde_json::Value = serde_json::from_str(&insp_body).expect("inspect JSON");
    assert_eq!(insp["inspectedFiles"], 1);
    assert!(insp["perLanguageMicros"]["rust"].as_u64().is_some());

    let (h1_status, h1_body) = raw_http_call(addr, "GET", "/health", None, "");
    assert_eq!(h1_status, 200);
    let h1: serde_json::Value = serde_json::from_str(&h1_body).expect("health after rust");
    assert_eq!(
        h1["activeRuntimeCount"], 1,
        "only `rust` should be warm while unused languages stay dormant: {h1}"
    );

    // Wait past 1s idle timeout -> idle reaper evicts `rust` back to dormant!
    let evict_deadline = Instant::now() + Duration::from_secs(4);
    let mut evicted = false;
    while Instant::now() < evict_deadline {
        let (_, hb) = raw_http_call(addr, "GET", "/health", None, "");
        let hj: serde_json::Value = serde_json::from_str(&hb).expect("health poll");
        if hj["activeRuntimeCount"] == 0 {
            evicted = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        evicted,
        "idle reaper must evict unused rust provider back to dormant"
    );

    // Close parent stdin pipe WITHOUT calling POST /shutdown -> stdin-EOF watchdog
    // must terminate `asp-gateway` cleanly within 2 seconds!
    drop(stdin);
    let exit_deadline = Instant::now() + Duration::from_secs(2);
    let mut exited = false;
    while Instant::now() < exit_deadline {
        if let Ok(Some(_)) = child.try_wait() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    if !exited {
        let _ = child.kill();
    }
    let _ = child.wait();
    assert!(
        exited,
        "stdin-EOF parent-death watchdog must terminate asp-gateway cleanly"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_single_flight_blake3_dedup_coalesces_16_agents_into_1_compiler_call() {
    struct SlowCountedProvider {
        pid: AstProviderId,
        lid: AstLanguageId,
        calls: Arc<AtomicUsize>,
    }

    impl AstProviderPort for SlowCountedProvider {
        fn provider_id(&self) -> &AstProviderId {
            &self.pid
        }
        fn language_id(&self) -> &AstLanguageId {
            &self.lid
        }
        fn lexical_profile(&self) -> LexicalDelimiterProfile {
            LexicalDelimiterProfile::rust()
        }
        fn inspect(
            &self,
            request: &AstInspectRequest,
        ) -> Result<AstInspectResponse, AstEngineError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(60));
            Ok(AstInspectResponse {
                accepted: true,
                nodes: vec![AstNodeSpanRecord {
                    locator: AstNodeLocator::new(format!("{}#fn:shared", request.path.as_str()))
                        .expect("loc"),
                    kind: AstUniversalKind::Function,
                    language_kind: AstLanguageKind::new("fn_decl").expect("lk"),
                    name: "shared".to_owned(),
                    line: 1,
                    outer_span: ByteSpan::new(0, request.source_utf8.len()).expect("span"),
                    inner_span: ByteSpan::new(0, request.source_utf8.len()).expect("span"),
                    body_span: None,
                    parent: None,
                    attrs: Vec::new(),
                }],
                edges: Vec::new(),
                diagnostics: Vec::new(),
                syntax_surface: None,
            })
        }
        fn verify(&self, _request: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
            Ok(AstVerifyResponse {
                results: Vec::new(),
            })
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let gateway = Arc::new(AspForwardingGateway::new());
    gateway
        .register_embedded_provider(
            demo_manifest("dedup", ".ddp"),
            Arc::new(SlowCountedProvider {
                pid: AstProviderId::new("asp-dedup").expect("pid"),
                lid: AstLanguageId::new("dedup").expect("lid"),
                calls: Arc::clone(&calls),
            }),
            None,
            None,
        )
        .expect("register dedup provider");

    // 16 concurrent agents request inspection of the exact same file simultaneously!
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let gw = Arc::clone(&gateway);
        set.spawn(async move {
            gw.inspect_files_to_open_edb(ForwardedInspectFilesRequest {
                files: vec![ForwardedInspectFileInput {
                    path: "src/shared.ddp".to_owned(),
                    source_utf8: "fn shared() { 42 }".to_owned(),
                    language_override: None,
                }],
            })
            .await
            .expect("inspect")
        });
    }
    while let Some(res) = set.join_next().await {
        let resp = res.expect("join");
        assert_eq!(resp.inspected_files, 1);
        assert_eq!(resp.total_entities, 1);
    }

    // All 16 concurrent callers coalesced into EXACTLY ONE native compiler invocation!
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let stats = gateway.inspect_cache_stats();
    assert_eq!(stats.cache_misses, 1);
    assert_eq!(stats.single_flight_coalesced + stats.cache_hits, 15);

    // Next sequential call hits the warm BLAKE3 cache with 0 compiler invocations!
    let cached_resp = gateway
        .inspect_files_to_open_edb(ForwardedInspectFilesRequest {
            files: vec![ForwardedInspectFileInput {
                path: "src/shared.ddp".to_owned(),
                source_utf8: "fn shared() { 42 }".to_owned(),
                language_override: None,
            }],
        })
        .await
        .expect("cached inspect");
    assert_eq!(cached_resp.total_entities, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(gateway.inspect_cache_stats().cache_hits >= 1);
}

#[test]
fn gateway_catalog_forwards_universal_protocol_schema_and_merges_provider_special_extensions() {
    use agent_semantic_provider_protocol::{
        OntologyKindMapping, ProviderProjectSurface, ProviderSpecialExtension,
    };

    let mut manifest = demo_manifest("zig", ".zig");
    manifest.project = Some(ProviderProjectSurface {
        markers: vec!["build.zig".to_owned()],
        entry_files: vec!["src/root.zig".to_owned(), "src/main.zig".to_owned()],
        workspace_files: vec!["build.zig.zon".to_owned()],
        config_files: Vec::new(),
    });
    manifest.special = Some(ProviderSpecialExtension {
        kind_mappings: vec![OntologyKindMapping {
            native_language_kind: "fn_proto".to_owned(),
            general_kind: "function".to_owned(),
            description: "Zig function declaration".to_owned(),
        }],
        special_relations: vec!["zig_comptime".to_owned()],
        schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "comptimeMode": {
                    "type": "string",
                    "enum": ["inline", "generic"]
                }
            }
        })),
    });

    let gateway = AspForwardingGateway::new();
    gateway
        .register_descriptor_only(manifest, None, None)
        .expect("register descriptor with special extension");

    let catalog = gateway.catalog().expect("catalog");
    let universal = catalog
        .universal_protocol_schema
        .as_ref()
        .expect("GET /v1/providers must forward ASP universalProtocolSchema");
    assert!(universal.pointer("/properties/exclude-tests?").is_some());
    assert!(universal.pointer("/properties/path-prefix").is_some());
    assert!(universal.pointer("/properties/kind").is_some());

    let zig_env = catalog
        .providers
        .iter()
        .find(|p| p.manifest.language_id.as_str() == "zig")
        .expect("zig envelope");
    assert_eq!(
        zig_env.targeted_schema.pointer("/properties/comptimeMode/enum"),
        Some(&serde_json::json!(["inline", "generic"]))
    );
    assert_eq!(
        zig_env.targeted_schema.pointer("/properties/languageKind/enum"),
        Some(&serde_json::json!(["fn_proto"]))
    );
    let pack = zig_env
        .dsl_pack
        .as_ref()
        .expect("manifest.special should synthesize dslPack automatically");
    assert_eq!(pack.special_relations, vec!["zig_comptime"]);

    // Verify all 5 checked-in language provider manifests decode with unified `project` + `special`.
    let asp_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for rel in [
        "languages/asp-rust/provider/provider-core-manifest.json",
        "languages/asp-python/provider/provider-core-manifest.json",
        "languages/asp-typescript/provider/provider-core-manifest.json",
        "languages/AspJulia.jl/juliac/provider-core-manifest.json",
        "languages/asp-gerbil-scheme/provider/provider-core-manifest.json",
    ] {
        let raw = std::fs::read_to_string(asp_root.join(rel))
            .unwrap_or_else(|e| panic!("read {rel}: {e}"));
        let m: ProviderCoreManifest =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("decode {rel}: {e}"));
        assert!(m.project.is_some(), "missing unified project surface in {rel}");
        assert!(m.special.is_some(), "missing special extension in {rel}");
    }
}
