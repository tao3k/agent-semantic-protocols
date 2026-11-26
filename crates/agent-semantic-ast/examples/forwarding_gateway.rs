// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runnable 5-Language ASP HTTP Forwarding Gateway (`forwarding_gateway`).
//!
//! Spawns all 5 real external native compiler HTTP-JSON providers (`asp-rust`,
//! `asp-python`, `asp-typescript`, `AspJulia.jl`, `asp-gerbil-scheme`), registers
//! them purely from `provider-core-manifest.json` + `schemas/conformance/*.json`,
//! prints a single JSON bootstrap line to stdout, and serves `/v1/providers`,
//! `/v1/providers/register`, `/v1/open-edb/inspect-files`, and `/shutdown` over
//! loopback HTTP/1.1 for Marlin's end-to-end search integration and benchmarks.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use agent_semantic_ast::AspForwardingGateway;
use agent_semantic_provider_protocol::{LanguageConformancePack, ProviderCoreManifest};
use tokio::net::TcpListener;
use tokio::sync::watch;

struct LiveProviderGuard {
    child: Child,
    _stdout: BufReader<std::process::ChildStdout>,
    endpoint: String,
}

fn post_shutdown(endpoint: &str) {
    let Some(authority) = endpoint
        .strip_prefix("http://")
        .and_then(|rest| rest.split('/').next())
    else {
        return;
    };
    let Ok(mut stream) = TcpStream::connect(authority) else {
        return;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let req = format!(
        "POST /shutdown HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
    );
    let _ = stream.write_all(req.as_bytes());
    let _ = stream.flush();
}

impl Drop for LiveProviderGuard {
    fn drop(&mut self) {
        post_shutdown(&self.endpoint);
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        #[cfg(unix)]
        {
            let pgid = format!("-{}", self.child.id());
            let _ = Command::new("/bin/kill").args(["-KILL", &pgid]).status();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn asp_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ASP repo root")
        .to_path_buf()
}

fn dummy_digest(tag: &str) -> String {
    format!("blake3-256:{}", blake3::hash(tag.as_bytes()).to_hex())
}

fn spawn_provider(mut cmd: Command) -> Result<LiveProviderGuard, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn provider: {e}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "missing provider stdout".to_owned())?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .map_err(|e| format!("read bootstrap: {e}"))?;
    if bytes == 0 {
        return Err("provider exited before emitting bootstrap line".to_owned());
    }
    let bootstrap: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|e| format!("parse bootstrap: {e}"))?;
    let endpoint = bootstrap["endpoint"]
        .as_str()
        .ok_or_else(|| "missing endpoint in bootstrap".to_owned())?
        .trim_end_matches('/')
        .to_owned();
    Ok(LiveProviderGuard {
        child,
        _stdout: reader,
        endpoint,
    })
}

fn load_manifest(path: &Path) -> Result<ProviderCoreManifest, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("decode {}: {e}", path.display()))
}

fn load_pack(path: &Path) -> Option<LanguageConformancePack> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn spawn_rust(root: &Path) -> Option<LiveProviderGuard> {
    let bin = root.join("languages/asp-rust/target/debug/asp-rust");
    if !bin.exists() {
        return None;
    }
    let mut cmd = Command::new(bin);
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
    spawn_provider(cmd).ok()
}

fn spawn_python(root: &Path) -> Option<LiveProviderGuard> {
    let src = root.join("languages/asp-python/src");
    if !src.join("asp_python/_ast_ops.py").exists() {
        return None;
    }
    let ops = serde_json::json!([
        {"operation": "ast-inspect", "requestSchema": {"schemaId": "agent.semantic-protocols.ast-inspect", "schemaVersion": "1"}, "responseSchema": {"schemaId": "agent.semantic-protocols.ast-inspect", "schemaVersion": "1"}},
        {"operation": "ast-verify", "requestSchema": {"schemaId": "agent.semantic-protocols.ast-verify", "schemaVersion": "1"}, "responseSchema": {"schemaId": "agent.semantic-protocols.ast-verify", "schemaVersion": "1"}}
    ]);
    let mut cmd = Command::new("python3");
    cmd.args(["-m", "asp_python", "serve"])
        .env("PYTHONPATH", src)
        .env("ASP_PROVIDER_ID", "asp-python")
        .env("ASP_PROVIDER_LANGUAGE_ID", "python")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("py-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("py-reg"))
        .env("ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST", dummy_digest("py-ctr"))
        .env("ASP_PROVIDER_RUNTIME_OPERATIONS_JSON", ops.to_string())
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    spawn_provider(cmd).ok()
}

fn spawn_typescript(root: &Path) -> Option<LiveProviderGuard> {
    let entry = root.join("languages/asp-typescript/dist/src/cli/main.js");
    if !entry.exists() {
        return None;
    }
    let mut cmd = Command::new("node");
    cmd.arg(entry)
        .arg("serve")
        .env("ASP_PROVIDER_ID", "asp-typescript")
        .env("ASP_PROVIDER_LANGUAGE_ID", "typescript")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("ts-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("ts-reg"))
        .env("ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST", dummy_digest("ts-ctr"))
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    spawn_provider(cmd).ok()
}

fn spawn_julia(root: &Path) -> Option<LiveProviderGuard> {
    let proj = root.join("languages/AspJulia.jl");
    if !proj.join("src/asp_client_server/ast_ops.jl").exists() {
        return None;
    }
    let julia_bin = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".juliaup/bin/julia"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("julia"));
    let mut cmd = Command::new(julia_bin);
    cmd.arg(format!("--project={}", proj.display()))
        .arg("-e")
        .arg("using AspJulia; AspJulia.run_asp_client_server()")
        .env("ASP_PROVIDER_ID", "asp-julia")
        .env("ASP_PROVIDER_LANGUAGE_ID", "julia")
        .env("ASP_PROVIDER_ARTIFACT_DIGEST", dummy_digest("jl-art"))
        .env("ASP_PROVIDER_REGISTRATION_DIGEST", dummy_digest("jl-reg"))
        .env("ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST", dummy_digest("jl-ctr"))
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    spawn_provider(cmd).ok()
}

fn spawn_scheme(root: &Path) -> Option<LiveProviderGuard> {
    let script = root.join("languages/asp-gerbil-scheme/src/runtime/ast-provider-http-server.ss");
    if !script.exists() {
        return None;
    }
    let mut cmd = Command::new("gxi");
    cmd.arg(script)
        .env("ASP_PROVIDER_ID", "asp-gerbil-scheme")
        .env("ASP_PROVIDER_LANGUAGE_ID", "gerbil-scheme")
        .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
    spawn_provider(cmd).ok()
}

type ProviderSpawnSpec<'a> = (&'a str, &'a str, &'a str, fn(&Path) -> Option<LiveProviderGuard>);

#[tokio::main]
async fn main() -> Result<(), String> {
    let root = asp_root();
    let gateway = Arc::new(AspForwardingGateway::new());
    let mut guards = Vec::new();
    let specs: Vec<ProviderSpawnSpec<'_>> = vec![
        (
            "rust",
            "languages/asp-rust/provider/provider-core-manifest.json",
            "schemas/conformance/rust.json",
            spawn_rust,
        ),
        (
            "python",
            "languages/asp-python/provider/provider-core-manifest.json",
            "schemas/conformance/python.json",
            spawn_python,
        ),
        (
            "typescript",
            "languages/asp-typescript/provider/provider-core-manifest.json",
            "schemas/conformance/typescript.json",
            spawn_typescript,
        ),
        (
            "julia",
            "languages/AspJulia.jl/juliac/provider-core-manifest.json",
            "schemas/conformance/julia.json",
            spawn_julia,
        ),
        (
            "gerbil-scheme",
            "languages/asp-gerbil-scheme/provider/provider-core-manifest.json",
            "schemas/conformance/gerbil-scheme.json",
            spawn_scheme,
        ),
    ];

    for (_lang, manifest_rel, pack_rel, spawner) in specs {
        if let Some(guard) = spawner(&root) {
            let manifest = load_manifest(&root.join(manifest_rel))?;
            let pack = load_pack(&root.join(pack_rel));
            gateway
                .register_http_provider(manifest, guard.endpoint.clone(), pack, None)
                .map_err(|e| e.to_string())?;
            guards.push(guard);
        }
    }

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("bind gateway: {e}"))?;
    let addr = listener.local_addr().map_err(|e| format!("addr: {e}"))?;
    let endpoint = format!("http://{addr}");
    let cat = gateway.catalog().map_err(|e| e.to_string())?;
    let bootstrap = serde_json::json!({
        "schemaId": "agent.semantic-protocols.asp-forwarding-gateway-bootstrap",
        "schemaVersion": "1",
        "endpoint": endpoint,
        "providers": cat.providers.iter().map(|p| p.manifest.language_id.as_str()).collect::<Vec<_>>(),
    });
    println!("{bootstrap}");
    let _ = std::io::stdout().flush();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let result = gateway.serve(listener, shutdown_tx, shutdown_rx).await;
    for guard in &guards {
        post_shutdown(&guard.endpoint);
    }
    drop(guards);
    result
}
