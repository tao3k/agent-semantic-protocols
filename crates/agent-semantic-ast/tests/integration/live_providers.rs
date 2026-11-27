// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Live end-to-end 5-language AI-Native Language Server Runtime integration &
//! black-box Provider Conformance Harness suite (RFC 10.35 + RFC 10.36):
//!
//! Spawns ALL FIVE real external native language compiler HTTP-JSON servers
//! simultaneously (`POST /v1/provider-runtime`), registers them into ONE
//! language-agnostic `AstProviderRegistry` (ZERO embedded parsers, ZERO hardcoded
//! Coding Style rules), and verifies:
//!   1. `languages/asp-rust` (`syn` v3.0.4 over HTTP-JSON — reports `//`)
//!   2. `languages/asp-python` (Python 3 stdlib `ast` over HTTP-JSON — reports `#`)
//!   3. `languages/asp-typescript` (TypeScript `tsc` over HTTP-JSON — reports `//`)
//!   4. `languages/AspJulia.jl` (`JuliaSyntax.jl` over HTTP-JSON — reports `#`)
//!   5. `languages/asp-gerbil-scheme` (Gerbil 0.19 native reader over HTTP-JSON — reports `;`)
//!
//! Every provider is verified against its Scheme-projected
//! `schemas/conformance/<lang>.json` pack (`defasp-language` POO General Ontology)
//! AND exercised on lossless trivia-preserving AST patching, Frontier 2 native
//! compiler oracle auto-calibration, and live single-tree Open EDB sync!

use agent_semantic_ast::{
    AstCalibrationOutcome, AstEngine, AstNodeLocator, AstPatchOp, AstPatchRequest, AstPatchStatus,
    AstWorkspacePath, LexicalDelimiterProfile, blake3_source_digest, run_language_conformance_pack,
};
use agent_semantic_provider_protocol::{LanguageConformancePack, ProviderCoreManifest};
use agent_semantic_search::WorkspaceSearchIndex;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;

struct LiveProviderGuard {
    child: Child,
    _stdout: BufReader<std::process::ChildStdout>,
    endpoint: String,
}

impl Drop for LiveProviderGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn asp_workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ASP repo root")
        .to_path_buf()
}

fn spawn_live_provider(mut cmd: Command) -> LiveProviderGuard {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn external provider process");
    let stdout = child.stdout.take().expect("provider stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).expect("read bootstrap JSON");
    assert!(bytes > 0, "provider exited before emitting bootstrap line");
    let bootstrap: serde_json::Value =
        serde_json::from_str(line.trim()).expect("decode provider bootstrap JSON");
    let endpoint = bootstrap["endpoint"]
        .as_str()
        .expect("bootstrap endpoint")
        .trim_end_matches('/')
        .to_owned();
    LiveProviderGuard {
        child,
        _stdout: reader,
        endpoint,
    }
}

fn dummy_digest(tag: &str) -> String {
    format!("blake3-256:{}", blake3::hash(tag.as_bytes()).to_hex())
}

fn spawn_asp_rust(root: &Path) -> Option<LiveProviderGuard> {
    let bin = root.join("languages/asp-rust/target/debug/asp-rust");
    if !bin.exists() {
        return None;
    }
    let mut cmd = Command::new(&bin);
    cmd.arg("serve")
        .env("ASP_PROVIDER_ID", "asp-rust")
        .env("ASP_PROVIDER_LANGUAGE_ID", "rust")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("rust-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("rust-reg"))
        .env(
            "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
            dummy_digest("rust-ctr"),
        )
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    Some(spawn_live_provider(cmd))
}

fn spawn_asp_python(root: &Path) -> Option<LiveProviderGuard> {
    let src_dir = root.join("languages/asp-python/src");
    if !src_dir.join("asp_python/_ast_ops.py").exists() {
        return None;
    }
    let ops_json = serde_json::json!([
        {
            "operation": "ast-inspect",
            "requestSchema": { "schemaId": "agent.semantic-protocols.ast-inspect", "schemaVersion": "1" },
            "responseSchema": { "schemaId": "agent.semantic-protocols.ast-inspect", "schemaVersion": "1" }
        },
        {
            "operation": "ast-verify",
            "requestSchema": { "schemaId": "agent.semantic-protocols.ast-verify", "schemaVersion": "1" },
            "responseSchema": { "schemaId": "agent.semantic-protocols.ast-verify", "schemaVersion": "1" }
        }
    ]);
    let mut cmd = Command::new("python3");
    cmd.args(["-m", "asp_python", "serve"])
        .env("PYTHONPATH", src_dir)
        .env("ASP_PROVIDER_ID", "asp-python")
        .env("ASP_PROVIDER_LANGUAGE_ID", "python")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("py-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("py-reg"))
        .env(
            "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
            dummy_digest("py-ctr"),
        )
        .env("ASP_PROVIDER_RUNTIME_OPERATIONS_JSON", ops_json.to_string())
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    Some(spawn_live_provider(cmd))
}

fn spawn_asp_typescript(root: &Path) -> Option<LiveProviderGuard> {
    let entry = root.join("languages/asp-typescript/dist/src/cli/main.js");
    if !entry.exists() {
        return None;
    }
    let mut cmd = Command::new("node");
    cmd.arg(&entry)
        .arg("serve")
        .env("ASP_PROVIDER_ID", "asp-typescript")
        .env("ASP_PROVIDER_LANGUAGE_ID", "typescript")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("ts-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("ts-reg"))
        .env(
            "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
            dummy_digest("ts-ctr"),
        )
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    Some(spawn_live_provider(cmd))
}

fn spawn_asp_julia(root: &Path) -> Option<LiveProviderGuard> {
    let project_dir = root.join("languages/AspJulia.jl");
    if !project_dir
        .join("src/asp_client_server/ast_ops.jl")
        .exists()
    {
        return None;
    }
    let julia_bin = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".juliaup/bin/julia"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("julia"));
    let mut cmd = Command::new(&julia_bin);
    cmd.arg(format!("--project={}", project_dir.display()))
        .arg("-e")
        .arg("using AspJulia; AspJulia.run_asp_client_server()")
        .env("ASP_PROVIDER_ID", "asp-julia")
        .env("ASP_PROVIDER_LANGUAGE_ID", "julia")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("jl-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("jl-reg"))
        .env(
            "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
            dummy_digest("jl-ctr"),
        )
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    Some(spawn_live_provider(cmd))
}

fn spawn_asp_gerbil_scheme(root: &Path) -> Option<LiveProviderGuard> {
    let script = root.join("languages/asp-gerbil-scheme/src/runtime/ast-provider-http-server.ss");
    if !script.is_file() {
        return None;
    }
    let mut cmd = Command::new("gxi");
    cmd.arg(&script)
        .env("ASP_PROVIDER_ID", "asp-gerbil-scheme")
        .env("ASP_PROVIDER_LANGUAGE_ID", "gerbil-scheme")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("scm-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("scm-reg"))
        .env(
            "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
            dummy_digest("scm-ctr"),
        )
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    Some(spawn_live_provider(cmd))
}

fn register_provider_from_manifest(
    engine: &mut AstEngine,
    root: &Path,
    manifest_rel_path: &str,
    endpoint: &str,
) {
    let path = root.join(manifest_rel_path);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read provider core manifest {}: {err}", path.display()));
    let manifest: ProviderCoreManifest = serde_json::from_str(&raw)
        .unwrap_or_else(|err| panic!("decode provider core manifest {}: {err}", path.display()));
    engine
        .providers_mut()
        .register_http_from_manifest(&manifest, endpoint)
        .expect("register live provider from manifest");
}

fn load_conformance_pack(root: &Path, provider_lang: &str) -> LanguageConformancePack {
    let path = root.join(format!("schemas/conformance/{provider_lang}.json"));
    let raw = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "read Scheme-projected conformance pack {}: {err}",
            path.display()
        )
    });
    serde_json::from_str(&raw)
        .unwrap_or_else(|err| panic!("decode conformance pack {}: {err}", path.display()))
}

#[test]
fn live_five_language_native_compilers_conformance_and_patching_over_http_json() {
    let root = asp_workspace_root();
    let Some(rust_guard) = spawn_asp_rust(&root) else {
        return;
    };
    let Some(py_guard) = spawn_asp_python(&root) else {
        return;
    };
    let Some(ts_guard) = spawn_asp_typescript(&root) else {
        return;
    };
    let Some(jl_guard) = spawn_asp_julia(&root) else {
        return;
    };
    let Some(scm_guard) = spawn_asp_gerbil_scheme(&root) else {
        return;
    };

    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join("src")).expect("mkdir src");
    let rust_orig = concat!(
        "// Copyright 2026 — untouched header\n",
        "/// Preserved Rust doc comment\n",
        "pub fn transform(opt: Option<i32>) -> Option<i32> {\n",
        "    opt\n",
        "}\n",
        "\n",
        "// Untouched sibling helper\n",
        "pub fn keep_me() -> i32 { 7 }\n",
    );
    let py_orig = concat!(
        "# Preserved Python module header\n",
        "def keep_py():\n",
        "    return 1\n",
        "\n",
        "def compute_py(x):\n",
        "    return x + 1\n",
    );
    let ts_orig = concat!(
        "// Preserved TS header\n",
        "export function keepTs(): number { return 1; }\n",
        "\n",
        "/** Preserved JSDoc */\n",
        "export function computeTs(x: number): number {\n",
        "    return x + 1;\n",
        "}\n",
    );
    let jl_orig = concat!(
        "# Preserved Julia header\n",
        "function keep_jl(x)\n",
        "    return x\n",
        "end\n",
        "\n",
        "function compute_jl(x)\n",
        "    return (x + 1)\n",
        "end\n",
    );
    let scm_orig = concat!(
        ";; Preserved Gerbil Scheme header\n",
        "(def (keep-scm x)\n",
        "  x)\n",
        "\n",
        "(def (compute-scm x)\n",
        "  (+ x 1))\n",
    );
    fs::write(tmp.path().join("src/lib.rs"), rust_orig).expect("write rs");
    fs::write(tmp.path().join("src/app.py"), py_orig).expect("write py");
    fs::write(tmp.path().join("src/mod.ts"), ts_orig).expect("write ts");
    fs::write(tmp.path().join("src/math.jl"), jl_orig).expect("write jl");
    fs::write(tmp.path().join("src/core.ss"), scm_orig).expect("write ss");

    let mut engine = AstEngine::new(tmp.path().to_path_buf());
    register_provider_from_manifest(
        &mut engine,
        &root,
        "languages/asp-rust/provider/provider-core-manifest.json",
        &rust_guard.endpoint,
    );
    register_provider_from_manifest(
        &mut engine,
        &root,
        "languages/asp-python/provider/provider-core-manifest.json",
        &py_guard.endpoint,
    );
    register_provider_from_manifest(
        &mut engine,
        &root,
        "languages/asp-typescript/provider/provider-core-manifest.json",
        &ts_guard.endpoint,
    );
    register_provider_from_manifest(
        &mut engine,
        &root,
        "languages/AspJulia.jl/juliac/provider-core-manifest.json",
        &jl_guard.endpoint,
    );
    register_provider_from_manifest(
        &mut engine,
        &root,
        "languages/asp-gerbil-scheme/provider/provider-core-manifest.json",
        &scm_guard.endpoint,
    );

    // 0. Black-Box Conformance Harness over ALL FIVE Scheme-projected packs!
    //    Verifies provider-reported `syntaxSurface` (`//`, `#`, `;`) and General
    //    Ontology mappings without duplicating lexical rules in Scheme.
    for (lang, expected_comment) in [
        ("rust", "//"),
        ("python", "#"),
        ("typescript", "//"),
        ("julia", "#"),
        ("gerbil-scheme", ";"),
    ] {
        let pack = load_conformance_pack(&root, lang);
        let report = run_language_conformance_pack(&engine, &pack)
            .unwrap_or_else(|err| panic!("run conformance pack for `{lang}`: {err}"));
        assert!(
            report.is_conformant(),
            "provider `{lang}` failed Scheme-projected conformance pack: {report:#?}"
        );
        let surface = report
            .syntax_surface
            .as_ref()
            .expect("provider must report native syntaxSurface");
        assert_eq!(
            surface.line_comment.as_deref(),
            Some(expected_comment),
            "provider `{lang}` reported unexpected comment syntax"
        );
        let derived_profile = LexicalDelimiterProfile::from_syntax_surface(surface);
        assert_eq!(derived_profile.line_comment.as_ref(), expected_comment);
    }

    let mut index = WorkspaceSearchIndex::build(tmp.path());

    // 1. Live `asp-rust` (`syn` v3 over HTTP): missing `}` calibrated + trivia preserved + Open EDB
    let rust_receipt = engine
        .apply_patch_with_index(
            &AstPatchRequest {
                path: AstWorkspacePath::new("src/lib.rs").expect("path"),
                language_id: None,
                expected_preimage_hash: Some(blake3_source_digest(rust_orig.as_bytes())),
                operation: AstPatchOp::ReplaceItem {
                    target: AstNodeLocator::new("src/lib.rs#fn:transform").expect("loc"),
                    snippet: "pub fn transform(opt: Option<i32>) -> Option<i32> {\n    opt.map(|v| v + 1)\n"
                        .to_owned(),
                },
                dry_run: false,
                auto_calibrate: true,
            },
            &mut index,
        )
        .expect("live asp-rust patch");
    assert_eq!(rust_receipt.status, AstPatchStatus::CalibratedAndApplied);
    assert_eq!(rust_receipt.provider_id.as_str(), "asp-rust");
    assert_eq!(
        rust_receipt.calibration.as_ref().expect("cal").outcome,
        AstCalibrationOutcome::CalibratedUniqueMinCost
    );
    assert!(rust_receipt.open_edb_ingested_entities >= 2);
    let updated_rs = fs::read_to_string(tmp.path().join("src/lib.rs")).expect("read rs");
    assert!(updated_rs.contains("// Copyright 2026 — untouched header"));
    assert!(updated_rs.contains("/// Preserved Rust doc comment"));
    assert!(updated_rs.contains("opt.map(|v| v + 1)"));

    // 2. Live `asp-python` (stdlib `ast` over HTTP): missing `)` calibrated + comment preserved
    let py_receipt = engine
        .apply_patch_with_index(
            &AstPatchRequest {
                path: AstWorkspacePath::new("src/app.py").expect("path"),
                language_id: None,
                expected_preimage_hash: None,
                operation: AstPatchOp::ReplaceItem {
                    target: AstNodeLocator::new("src/app.py#fn:compute_py").expect("loc"),
                    snippet: "def compute_py(x):\n    return (x * 10 + 1".to_owned(),
                },
                dry_run: false,
                auto_calibrate: true,
            },
            &mut index,
        )
        .expect("live asp-python patch");
    assert_eq!(py_receipt.status, AstPatchStatus::CalibratedAndApplied);
    assert_eq!(py_receipt.provider_id.as_str(), "asp-python");
    let updated_py = fs::read_to_string(tmp.path().join("src/app.py")).expect("read py");
    assert!(updated_py.contains("# Preserved Python module header"));
    assert!(updated_py.contains("return (x * 10 + 1)"));

    // 3. Live `asp-typescript` (`tsc` 5.9 over HTTP): missing `}` calibrated + JSDoc preserved
    let ts_receipt = engine
        .apply_patch_with_index(
            &AstPatchRequest {
                path: AstWorkspacePath::new("src/mod.ts").expect("path"),
                language_id: None,
                expected_preimage_hash: None,
                operation: AstPatchOp::ReplaceItem {
                    target: AstNodeLocator::new("src/mod.ts#fn:computeTs").expect("loc"),
                    snippet: "export function computeTs(x: number): number {\n    return x * 3;\n"
                        .to_owned(),
                },
                dry_run: false,
                auto_calibrate: true,
            },
            &mut index,
        )
        .expect("live asp-typescript patch");
    assert_eq!(ts_receipt.status, AstPatchStatus::CalibratedAndApplied);
    assert_eq!(ts_receipt.provider_id.as_str(), "asp-typescript");
    let updated_ts = fs::read_to_string(tmp.path().join("src/mod.ts")).expect("read ts");
    assert!(updated_ts.contains("/** Preserved JSDoc */"));
    assert!(updated_ts.contains("return x * 3;"));

    // 4. Live `AspJulia.jl` (`JuliaSyntax.jl` over HTTP): lossless patch + missing `)` calibrated
    let jl_receipt = engine
        .apply_patch_with_index(
            &AstPatchRequest {
                path: AstWorkspacePath::new("src/math.jl").expect("path"),
                language_id: None,
                expected_preimage_hash: None,
                operation: AstPatchOp::ReplaceItem {
                    target: AstNodeLocator::new("src/math.jl#fn:compute_jl").expect("loc"),
                    snippet: "function compute_jl(x)\n    return (x * 4 + 2\nend".to_owned(),
                },
                dry_run: false,
                auto_calibrate: true,
            },
            &mut index,
        )
        .expect("live asp-julia patch");
    assert_eq!(jl_receipt.status, AstPatchStatus::CalibratedAndApplied);
    assert_eq!(jl_receipt.provider_id.as_str(), "asp-julia");
    let updated_jl = fs::read_to_string(tmp.path().join("src/math.jl")).expect("read jl");
    assert!(updated_jl.contains("# Preserved Julia header"));
    assert!(updated_jl.contains("function keep_jl(x)"));
    assert!(updated_jl.contains("x * 4 + 2)"));

    // 5. Live `asp-gerbil-scheme` (Gerbil 0.19 HTTP-JSON server): missing `)` calibrated + comment preserved
    let scm_receipt = engine
        .apply_patch_with_index(
            &AstPatchRequest {
                path: AstWorkspacePath::new("src/core.ss").expect("path"),
                language_id: None,
                expected_preimage_hash: None,
                operation: AstPatchOp::ReplaceItem {
                    target: AstNodeLocator::new("src/core.ss#fn:compute-scm").expect("loc"),
                    snippet: "(def (compute-scm x)\n  (* x 5)".to_owned(),
                },
                dry_run: false,
                auto_calibrate: true,
            },
            &mut index,
        )
        .expect("live asp-gerbil-scheme patch");
    assert_eq!(scm_receipt.status, AstPatchStatus::CalibratedAndApplied);
    assert_eq!(scm_receipt.provider_id.as_str(), "asp-gerbil-scheme");
    let updated_scm = fs::read_to_string(tmp.path().join("src/core.ss")).expect("read ss");
    assert!(updated_scm.contains(";; Preserved Gerbil Scheme header"));
    assert!(updated_scm.contains("(def (keep-scm x)"));
    assert!(updated_scm.contains("(* x 5))"));
}
