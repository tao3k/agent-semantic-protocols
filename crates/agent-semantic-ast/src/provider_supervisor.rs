// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Production Provider Process Supervision for `asp-gateway`:
//! - Lazy-on-demand single-flight spawning (zero boot cost for unused languages)
//! - Idle eviction reaper (releases memory of unused compilers while preserving catalog)
//! - AOT-first binary discovery (`asp-rust`, `asp-gerbil-scheme`, `JuliaC`)
//! - Process-group isolation + stdin-EOF parent-death watchdog (zero orphan providers)

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_semantic_provider_protocol::{LanguageConformancePack, ProviderCoreManifest};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::gateway_governance::{
    GatewaySessionToken, ProviderCircuitBreakerPolicy, ProviderLifecycleState, ProviderRuntimeKind,
    ProviderSpawnMode,
};
use crate::http_gateway::AspForwardingGateway;
use crate::provider_port::{
    AstInspectRequest, AstInspectResponse, AstProviderClientConfig, AstProviderPort,
    AstVerifyRequest, AstVerifyResponse, HttpJsonAstProviderClient,
};
use crate::types::{
    AstEngineError, AstLanguageId, AstProviderCapabilities, AstProviderId, LexicalDelimiterProfile,
};

const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Supervised external provider child process with process-group isolation
/// and kept-open stdin pipe so parent death delivers instant EOF.
pub struct LiveProviderGuard {
    child: Child,
    _stdin: Option<ChildStdin>,
    _stdout: BufReader<ChildStdout>,
    endpoint: String,
}

impl LiveProviderGuard {
    /// Loopback HTTP endpoint advertised by the child's bootstrap frame.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

/// Request cooperative shutdown of a loopback HTTP provider endpoint.
pub fn post_provider_shutdown(endpoint: &str) {
    let Some(authority) = endpoint
        .strip_prefix("http://")
        .and_then(|rest| rest.split('/').next())
    else {
        return;
    };
    let Ok(mut stream) = TcpStream::connect(authority) else {
        return;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let req = format!(
        "POST /shutdown HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
    );
    let _ = stream.write_all(req.as_bytes());
    let _ = stream.flush();
}

impl Drop for LiveProviderGuard {
    fn drop(&mut self) {
        let _ = self._stdin.take();
        post_provider_shutdown(&self.endpoint);
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

fn spawn_supervised_provider_process(mut cmd: Command) -> Result<LiveProviderGuard, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn provider: {e}"))?;
    let stdin = child.stdin.take();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "missing provider stdout".to_owned())?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .map_err(|e| format!("read provider bootstrap: {e}"))?;
    if bytes == 0 {
        return Err("provider exited before emitting bootstrap line".to_owned());
    }
    let bootstrap: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|e| format!("parse provider bootstrap: {e}"))?;
    let endpoint = bootstrap["endpoint"]
        .as_str()
        .ok_or_else(|| "missing endpoint in provider bootstrap".to_owned())?
        .trim_end_matches('/')
        .to_owned();
    Ok(LiveProviderGuard {
        child,
        _stdin: stdin,
        _stdout: reader,
        endpoint,
    })
}

fn deterministic_digest(tag: &str) -> String {
    format!("blake3-256:{}", blake3::hash(tag.as_bytes()).to_hex())
}

type ProviderSpawnFactory = Arc<dyn Fn() -> Result<LiveProviderGuard, String> + Send + Sync>;

/// Discovered native provider launch blueprint with AOT-first binary resolution.
pub struct DiscoveredProviderSpec {
    pub manifest: ProviderCoreManifest,
    pub dsl_pack: Option<LanguageConformancePack>,
    pub runtime_kind: ProviderRuntimeKind,
    spawn_factory: ProviderSpawnFactory,
}

struct ActiveManagedProvider {
    guard: LiveProviderGuard,
    client: HttpJsonAstProviderClient,
    last_used: Instant,
}

/// Lazy-on-demand supervised [`AstProviderPort`] supporting single-flight
/// spawning, exponential crash-restart backoff, PID/RSS tracking, and idle eviction.
pub struct LazySupervisedProviderPort {
    config: AstProviderClientConfig,
    runtime_kind: ProviderRuntimeKind,
    spawn_factory: ProviderSpawnFactory,
    gateway: Arc<AspForwardingGateway>,
    active: Mutex<Option<ActiveManagedProvider>>,
    restart_backoff: Mutex<(u32, Option<Instant>)>,
}

impl LazySupervisedProviderPort {
    /// Construct a dormant lazy provider port bound to `gateway` telemetry.
    #[must_use]
    pub fn new(
        config: AstProviderClientConfig,
        runtime_kind: ProviderRuntimeKind,
        spawn_factory: ProviderSpawnFactory,
        gateway: Arc<AspForwardingGateway>,
    ) -> Self {
        Self {
            config,
            runtime_kind,
            spawn_factory,
            gateway,
            active: Mutex::new(None),
            restart_backoff: Mutex::new((0, None)),
        }
    }

    /// Return the discovered runtime kind (`aot-binary`, `node-bundle`, `interpreter-script`).
    #[must_use]
    pub fn runtime_kind(&self) -> ProviderRuntimeKind {
        self.runtime_kind
    }

    /// Eagerly warm the child process right now if it is not already running.
    pub fn ensure_warm(&self) -> Result<(), AstEngineError> {
        let mut slot = self
            .active
            .lock()
            .map_err(|_| AstEngineError::ProviderError("provider slot poisoned".to_owned()))?;
        let _ = self.ensure_active_locked(&mut slot)?;
        Ok(())
    }

    /// Shut down the child process if it has been idle for at least `idle_timeout`.
    pub fn evict_if_idle(&self, idle_timeout: Duration) -> bool {
        let Ok(mut slot) = self.active.lock() else {
            return false;
        };
        let should_evict = slot
            .as_ref()
            .is_some_and(|live| live.last_used.elapsed() >= idle_timeout);
        if should_evict {
            let _ = slot.take();
            self.gateway.set_provider_lifecycle_state(
                &self.config.language_id,
                ProviderLifecycleState::Dormant,
                true,
                None,
            );
            true
        } else {
            false
        }
    }

    /// Immediately terminate any running child process owned by this port.
    pub fn shutdown_now(&self) {
        if let Ok(mut slot) = self.active.lock()
            && slot.take().is_some()
        {
            self.gateway.set_provider_lifecycle_state(
                &self.config.language_id,
                ProviderLifecycleState::Dormant,
                false,
                None,
            );
        }
    }

    fn check_restart_backoff(&self) -> Result<(), AstEngineError> {
        let Ok(backoff) = self.restart_backoff.lock() else {
            return Ok(());
        };
        let (failures, last_fail) = *backoff;
        if failures == 0 {
            return Ok(());
        }
        let delay_ms = 100u64
            .saturating_mul(1u64 << failures.saturating_sub(1).min(5))
            .min(5_000);
        if let Some(at) = last_fail
            && at.elapsed() < Duration::from_millis(delay_ms)
        {
            return Err(AstEngineError::ProviderError(format!(
                "provider `{}` in crash-restart backoff ({delay_ms}ms after {failures} spawn failures)",
                self.config.language_id.as_str()
            )));
        }
        Ok(())
    }

    fn record_spawn_outcome(&self, ok: bool) {
        if let Ok(mut backoff) = self.restart_backoff.lock() {
            if ok {
                *backoff = (0, None);
            } else {
                backoff.0 = backoff.0.saturating_add(1);
                backoff.1 = Some(Instant::now());
            }
        }
    }

    fn ensure_active_locked<'a>(
        &self,
        slot: &'a mut Option<ActiveManagedProvider>,
    ) -> Result<&'a mut ActiveManagedProvider, AstEngineError> {
        let needs_spawn = match slot.as_mut() {
            Some(live) => matches!(live.guard.child.try_wait(), Ok(Some(_))),
            None => true,
        };
        if needs_spawn {
            let _ = slot.take();
            self.check_restart_backoff()?;
            let guard = match (self.spawn_factory)() {
                Ok(g) => {
                    self.record_spawn_outcome(true);
                    g
                }
                Err(err) => {
                    self.record_spawn_outcome(false);
                    self.gateway.set_provider_lifecycle_state(
                        &self.config.language_id,
                        ProviderLifecycleState::Degraded,
                        false,
                        None,
                    );
                    return Err(AstEngineError::ProviderError(err));
                }
            };
            let pid = guard.child.id();
            let client = HttpJsonAstProviderClient::new(guard.endpoint(), self.config.clone());
            self.gateway.set_provider_lifecycle_state(
                &self.config.language_id,
                ProviderLifecycleState::Warm,
                false,
                Some(pid),
            );
            *slot = Some(ActiveManagedProvider {
                guard,
                client,
                last_used: Instant::now(),
            });
        }
        let live = slot
            .as_mut()
            .ok_or_else(|| AstEngineError::ProviderError("missing active provider".to_owned()))?;
        live.last_used = Instant::now();
        Ok(live)
    }
}

impl AstProviderPort for LazySupervisedProviderPort {
    fn provider_id(&self) -> &AstProviderId {
        &self.config.provider_id
    }

    fn language_id(&self) -> &AstLanguageId {
        &self.config.language_id
    }

    fn lexical_profile(&self) -> LexicalDelimiterProfile {
        self.config.lexical_profile.clone()
    }

    fn inspect(&self, request: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError> {
        let mut slot = self
            .active
            .lock()
            .map_err(|_| AstEngineError::ProviderError("provider slot poisoned".to_owned()))?;
        let live = self.ensure_active_locked(&mut slot)?;
        let res = live.client.inspect(request);
        if res.is_err() && matches!(live.guard.child.try_wait(), Ok(Some(_))) {
            let _ = slot.take();
        }
        res
    }

    fn verify(&self, request: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
        let mut slot = self
            .active
            .lock()
            .map_err(|_| AstEngineError::ProviderError("provider slot poisoned".to_owned()))?;
        let live = self.ensure_active_locked(&mut slot)?;
        let res = live.client.verify(request);
        if res.is_err() && matches!(live.guard.child.try_wait(), Ok(Some(_))) {
            let _ = slot.take();
        }
        res
    }
}

fn load_manifest(path: &Path) -> Result<ProviderCoreManifest, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("decode {}: {e}", path.display()))
}

fn load_pack(path: &Path) -> Option<LanguageConformancePack> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn build_rust_spec(root: &Path) -> Option<(ProviderRuntimeKind, ProviderSpawnFactory)> {
    let release_bin = root.join("languages/asp-rust/target/release/asp-rust");
    let debug_bin = root.join("languages/asp-rust/target/debug/asp-rust");
    let bin = if release_bin.exists() {
        release_bin
    } else if debug_bin.exists() {
        debug_bin
    } else {
        return None;
    };
    let factory: ProviderSpawnFactory = Arc::new(move || {
        let mut cmd = Command::new(&bin);
        cmd.arg("serve")
            .env("ASP_PROVIDER_ID", "asp-rust")
            .env("ASP_PROVIDER_LANGUAGE_ID", "rust")
            .env(
                "ASP_PROVIDER_ARTIFACT_DIGEST",
                deterministic_digest("rust-art"),
            )
            .env(
                "ASP_PROVIDER_REGISTRATION_DIGEST",
                deterministic_digest("rust-reg"),
            )
            .env(
                "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
                deterministic_digest("rust-ctr"),
            )
            .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
        spawn_supervised_provider_process(cmd)
    });
    Some((ProviderRuntimeKind::AotBinary, factory))
}

fn build_python_spec(root: &Path) -> Option<(ProviderRuntimeKind, ProviderSpawnFactory)> {
    let src = root.join("languages/asp-python/src");
    if !src.join("asp_python/_ast_ops.py").exists() {
        return None;
    }
    let factory: ProviderSpawnFactory = Arc::new(move || {
        let ops = serde_json::json!([
            {"operation": "ast-inspect", "requestSchema": {"schemaId": "agent.semantic-protocols.ast-inspect", "schemaVersion": "1"}, "responseSchema": {"schemaId": "agent.semantic-protocols.ast-inspect", "schemaVersion": "1"}},
            {"operation": "ast-verify", "requestSchema": {"schemaId": "agent.semantic-protocols.ast-verify", "schemaVersion": "1"}, "responseSchema": {"schemaId": "agent.semantic-protocols.ast-verify", "schemaVersion": "1"}}
        ]);
        let mut cmd = Command::new("python3");
        cmd.args(["-m", "asp_python", "serve"])
            .env("PYTHONPATH", &src)
            .env("ASP_PROVIDER_ID", "asp-python")
            .env("ASP_PROVIDER_LANGUAGE_ID", "python")
            .env(
                "ASP_PROVIDER_ARTIFACT_DIGEST",
                deterministic_digest("py-art"),
            )
            .env(
                "ASP_PROVIDER_REGISTRATION_DIGEST",
                deterministic_digest("py-reg"),
            )
            .env(
                "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
                deterministic_digest("py-ctr"),
            )
            .env("ASP_PROVIDER_RUNTIME_OPERATIONS_JSON", ops.to_string())
            .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
        spawn_supervised_provider_process(cmd)
    });
    Some((ProviderRuntimeKind::InterpreterScript, factory))
}

fn build_typescript_spec(root: &Path) -> Option<(ProviderRuntimeKind, ProviderSpawnFactory)> {
    let entry = root.join("languages/asp-typescript/dist/src/cli/main.js");
    if !entry.exists() {
        return None;
    }
    let factory: ProviderSpawnFactory = Arc::new(move || {
        let mut cmd = Command::new("node");
        cmd.arg(&entry)
            .arg("serve")
            .env("ASP_PROVIDER_ID", "asp-typescript")
            .env("ASP_PROVIDER_LANGUAGE_ID", "typescript")
            .env(
                "ASP_PROVIDER_ARTIFACT_DIGEST",
                deterministic_digest("ts-art"),
            )
            .env(
                "ASP_PROVIDER_REGISTRATION_DIGEST",
                deterministic_digest("ts-reg"),
            )
            .env(
                "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
                deterministic_digest("ts-ctr"),
            )
            .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
        spawn_supervised_provider_process(cmd)
    });
    Some((ProviderRuntimeKind::NodeBundle, factory))
}

fn build_julia_spec(root: &Path) -> Option<(ProviderRuntimeKind, ProviderSpawnFactory)> {
    let aot_override = std::env::var_os("ASP_JULIA_AOT_BIN")
        .map(PathBuf::from)
        .filter(|p| p.exists());
    let default_aot = root.join("languages/AspJulia.jl/juliac/build/bin/asp_julia");
    if let Some(aot_bin) = aot_override.or_else(|| default_aot.exists().then_some(default_aot)) {
        let factory: ProviderSpawnFactory = Arc::new(move || {
            let mut cmd = Command::new(&aot_bin);
            cmd.arg("serve")
                .env("ASP_PROVIDER_ID", "asp-julia")
                .env("ASP_PROVIDER_LANGUAGE_ID", "julia")
                .env(
                    "ASP_PROVIDER_ARTIFACT_DIGEST",
                    deterministic_digest("jl-art"),
                )
                .env(
                    "ASP_PROVIDER_REGISTRATION_DIGEST",
                    deterministic_digest("jl-reg"),
                )
                .env(
                    "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
                    deterministic_digest("jl-ctr"),
                )
                .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
            spawn_supervised_provider_process(cmd)
        });
        return Some((ProviderRuntimeKind::AotBinary, factory));
    }

    let proj = root.join("languages/AspJulia.jl");
    if !proj.join("src/asp_client_server/ast_ops.jl").exists() {
        return None;
    }
    let julia_bin = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".juliaup/bin/julia"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("julia"));
    let factory: ProviderSpawnFactory = Arc::new(move || {
        let mut cmd = Command::new(&julia_bin);
        cmd.arg(format!("--project={}", proj.display()))
            .arg("-e")
            .arg("using AspJulia; AspJulia.run_asp_client_server()")
            .env("ASP_PROVIDER_ID", "asp-julia")
            .env("ASP_PROVIDER_LANGUAGE_ID", "julia")
            .env(
                "ASP_PROVIDER_ARTIFACT_DIGEST",
                deterministic_digest("jl-art"),
            )
            .env(
                "ASP_PROVIDER_REGISTRATION_DIGEST",
                deterministic_digest("jl-reg"),
            )
            .env(
                "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST",
                deterministic_digest("jl-ctr"),
            )
            .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
        spawn_supervised_provider_process(cmd)
    });
    Some((ProviderRuntimeKind::InterpreterScript, factory))
}

fn build_scheme_spec(root: &Path) -> Option<(ProviderRuntimeKind, ProviderSpawnFactory)> {
    let aot_override = std::env::var_os("ASP_GERBIL_AST_SERVER_BIN")
        .map(PathBuf::from)
        .filter(|p| p.exists());
    let default_aot =
        root.join("languages/asp-gerbil-scheme/.gerbil/bin/asp-gerbil-scheme-ast-server");
    if let Some(aot_bin) = aot_override.or_else(|| default_aot.exists().then_some(default_aot)) {
        let factory: ProviderSpawnFactory = Arc::new(move || {
            let mut cmd = Command::new(&aot_bin);
            cmd.env("ASP_PROVIDER_ID", "asp-gerbil-scheme")
                .env("ASP_PROVIDER_LANGUAGE_ID", "gerbil-scheme")
                .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
            spawn_supervised_provider_process(cmd)
        });
        return Some((ProviderRuntimeKind::AotBinary, factory));
    }

    let script = root.join("languages/asp-gerbil-scheme/src/runtime/ast-provider-http-server.ss");
    if !script.exists() {
        return None;
    }
    let factory: ProviderSpawnFactory = Arc::new(move || {
        let mut cmd = Command::new("gxi");
        cmd.arg(&script)
            .env("ASP_PROVIDER_ID", "asp-gerbil-scheme")
            .env("ASP_PROVIDER_LANGUAGE_ID", "gerbil-scheme")
            .env("ASP_CLIENT_SERVER_HOST", "127.0.0.1:0");
        spawn_supervised_provider_process(cmd)
    });
    Some((ProviderRuntimeKind::InterpreterScript, factory))
}

type ProviderDiscoveryEntry<'a> = (
    &'a str,
    &'a str,
    fn(&Path) -> Option<(ProviderRuntimeKind, ProviderSpawnFactory)>,
);

/// Discover all locally available native language providers under `asp_root`,
/// preferring compiled AOT binaries over interpreted scripts.
pub fn discover_default_provider_specs(
    asp_root: &Path,
) -> Result<Vec<DiscoveredProviderSpec>, String> {
    let entries: [ProviderDiscoveryEntry<'_>; 5] = [
        (
            "languages/asp-rust/provider/provider-core-manifest.json",
            "schemas/conformance/rust.json",
            build_rust_spec,
        ),
        (
            "languages/asp-python/provider/provider-core-manifest.json",
            "schemas/conformance/python.json",
            build_python_spec,
        ),
        (
            "languages/asp-typescript/provider/provider-core-manifest.json",
            "schemas/conformance/typescript.json",
            build_typescript_spec,
        ),
        (
            "languages/AspJulia.jl/juliac/provider-core-manifest.json",
            "schemas/conformance/julia.json",
            build_julia_spec,
        ),
        (
            "languages/asp-gerbil-scheme/provider/provider-core-manifest.json",
            "schemas/conformance/gerbil-scheme.json",
            build_scheme_spec,
        ),
    ];

    let mut specs = Vec::new();
    for (manifest_rel, pack_rel, builder) in entries {
        let Some((runtime_kind, spawn_factory)) = builder(asp_root) else {
            continue;
        };
        let manifest_path = asp_root.join(manifest_rel);
        if !manifest_path.exists() {
            continue;
        }
        let manifest = load_manifest(&manifest_path)?;
        let dsl_pack = load_pack(&asp_root.join(pack_rel));
        specs.push(DiscoveredProviderSpec {
            manifest,
            dsl_pack,
            runtime_kind,
            spawn_factory,
        });
    }
    Ok(specs)
}

/// Launch configuration for the production `asp-gateway` daemon.
#[derive(Clone, Debug)]
pub struct GatewayLaunchConfig {
    pub asp_root: PathBuf,
    pub bind_addr: String,
    pub eager_boot: bool,
    pub idle_timeout: Duration,
    pub watch_stdin_eof: bool,
    pub require_control_token: bool,
    pub circuit_policy: ProviderCircuitBreakerPolicy,
}

impl GatewayLaunchConfig {
    /// Construct production defaults (`lazy-on-demand`, 300s idle eviction,
    /// loopback Bearer control token enabled, stdin-EOF watchdog enabled).
    #[must_use]
    pub fn production(asp_root: PathBuf) -> Self {
        let idle_secs = std::env::var("ASP_GATEWAY_IDLE_TIMEOUT_SECS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(DEFAULT_IDLE_TIMEOUT.as_secs());
        let bind_addr =
            std::env::var("ASP_GATEWAY_BIND").unwrap_or_else(|_| "127.0.0.1:0".to_owned());
        let eager_boot = std::env::var("ASP_GATEWAY_EAGER")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        Self {
            asp_root,
            bind_addr,
            eager_boot,
            idle_timeout: Duration::from_secs(idle_secs.max(1)),
            watch_stdin_eof: true,
            require_control_token: true,
            circuit_policy: ProviderCircuitBreakerPolicy::default(),
        }
    }
}

fn spawn_stdin_eof_watchdog(shutdown_tx: watch::Sender<bool>) {
    let _ = std::thread::Builder::new()
        .name("asp-gateway-stdin-watchdog".to_owned())
        .spawn(move || {
            let mut stdin = std::io::stdin().lock();
            let mut buf = [0u8; 64];
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                    Ok(_) => {}
                }
            }
        });
}

/// Parse `asp-gateway` CLI arguments (`--asp-root`, `--bind`, `--idle-timeout-secs`,
/// `--eager`, `--no-stdin-watchdog`, `--no-control-token`) into [`GatewayLaunchConfig`].
pub fn parse_gateway_cli_config<I>(args: I) -> Result<GatewayLaunchConfig, String>
where
    I: IntoIterator<Item = String>,
{
    let default_root = if let Some(env_root) = std::env::var_os("ASP_REPO_ROOT") {
        PathBuf::from(env_root)
    } else {
        let compile_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        compile_root
            .filter(|p| p.join("schemas/conformance").exists())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    };
    let mut cfg = GatewayLaunchConfig::production(default_root);
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--asp-root" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--asp-root requires a path".to_owned())?;
                cfg.asp_root = PathBuf::from(raw);
            }
            "--bind" => {
                cfg.bind_addr = iter
                    .next()
                    .ok_or_else(|| "--bind requires host:port".to_owned())?;
            }
            "--idle-timeout-secs" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--idle-timeout-secs requires integer seconds".to_owned())?;
                let secs = raw
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --idle-timeout-secs `{raw}`: {e}"))?;
                cfg.idle_timeout = Duration::from_secs(secs.max(1));
            }
            "--eager" => {
                cfg.eager_boot = true;
            }
            "--no-stdin-watchdog" => {
                cfg.watch_stdin_eof = false;
            }
            "--no-control-token" => {
                cfg.require_control_token = false;
            }
            other => {
                return Err(format!("unknown asp-gateway flag: `{other}`"));
            }
        }
    }
    Ok(cfg)
}

/// Entrypoint adapter invoked by the thin `asp-gateway` binary.
pub async fn run_asp_gateway_from_args<I>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = String>,
{
    let config = parse_gateway_cli_config(args)?;
    run_supervised_asp_gateway(config).await
}

/// Run the supervised production ASP forwarding gateway until `/shutdown` or stdin EOF.
pub async fn run_supervised_asp_gateway(config: GatewayLaunchConfig) -> Result<(), String> {
    let session_token = config
        .require_control_token
        .then(GatewaySessionToken::generate);
    let gateway = Arc::new(AspForwardingGateway::with_governance(
        config.circuit_policy,
        session_token.clone(),
    ));

    let specs = discover_default_provider_specs(&config.asp_root)?;
    let mut managed_ports: Vec<Arc<LazySupervisedProviderPort>> = Vec::with_capacity(specs.len());
    let spawn_mode = if config.eager_boot {
        ProviderSpawnMode::Eager
    } else {
        ProviderSpawnMode::LazyOnDemand
    };

    for spec in specs {
        let language_id =
            AstLanguageId::new(spec.manifest.language_id.as_str()).map_err(|e| e.to_string())?;
        let provider_id =
            AstProviderId::new(spec.manifest.provider_id.as_str()).map_err(|e| e.to_string())?;
        let client_cfg = AstProviderClientConfig {
            provider_id,
            language_id,
            lexical_profile: LexicalDelimiterProfile::from_syntax_surface(
                &spec.manifest.syntax_surface,
            ),
            capabilities: AstProviderCapabilities {
                ast_inspect: spec.manifest.capabilities.ast_inspect,
                ast_verify: spec.manifest.capabilities.ast_verify,
                open_edb_facts: spec.manifest.capabilities.open_edb,
            },
        };
        let port = Arc::new(LazySupervisedProviderPort::new(
            client_cfg,
            spec.runtime_kind,
            spec.spawn_factory,
            Arc::clone(&gateway),
        ));
        gateway
            .register_managed_provider(
                spec.manifest,
                Arc::clone(&port) as Arc<dyn AstProviderPort>,
                spec.dsl_pack,
                None,
                spec.runtime_kind,
                spawn_mode,
                ProviderLifecycleState::Dormant,
            )
            .map_err(|e| e.to_string())?;
        managed_ports.push(port);
    }

    if config.eager_boot {
        let mut warm_set = tokio::task::JoinSet::new();
        for port in &managed_ports {
            let p = Arc::clone(port);
            warm_set.spawn_blocking(move || p.ensure_warm());
        }
        while let Some(res) = warm_set.join_next().await {
            let _ = res;
        }
    }

    let listener = TcpListener::bind(&config.bind_addr)
        .await
        .map_err(|e| format!("bind asp-gateway on {}: {e}", config.bind_addr))?;
    let addr = listener.local_addr().map_err(|e| format!("addr: {e}"))?;
    let endpoint = format!("http://{addr}");
    let cat = gateway.catalog().map_err(|e| e.to_string())?;
    let health = gateway.health_receipts().map_err(|e| e.to_string())?;

    let mut bootstrap = serde_json::json!({
        "schemaId": "agent.semantic-protocols.asp-forwarding-gateway-bootstrap",
        "schemaVersion": "1",
        "endpoint": endpoint,
        "spawnMode": spawn_mode,
        "idleTimeoutSecs": config.idle_timeout.as_secs(),
        "providers": cat.providers.iter().map(|p| p.manifest.language_id.as_str()).collect::<Vec<_>>(),
        "providerHealth": health,
    });
    if let Some(token) = &session_token {
        bootstrap["controlToken"] = serde_json::Value::String(token.as_str().to_owned());
    }
    println!("{bootstrap}");
    let _ = std::io::stdout().flush();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    if config.watch_stdin_eof {
        spawn_stdin_eof_watchdog(shutdown_tx.clone());
    }

    let reaper_ports = managed_ports.clone();
    let idle_timeout = config.idle_timeout;
    let mut reaper_shutdown = shutdown_rx.clone();
    let reaper_task = tokio::spawn(async move {
        let tick = idle_timeout
            .min(Duration::from_secs(10))
            .max(Duration::from_millis(50));
        let mut interval = tokio::time::interval(tick);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    for port in &reaper_ports {
                        let _ = port.evict_if_idle(idle_timeout);
                    }
                }
                changed = reaper_shutdown.changed() => {
                    if changed.is_err() || *reaper_shutdown.borrow() {
                        break;
                    }
                }
            }
        }
    });

    let result = gateway.serve(listener, shutdown_tx, shutdown_rx).await;
    reaper_task.abort();
    for port in &managed_ports {
        port.shutdown_now();
    }
    result
}
