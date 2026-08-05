use std::path::{Path, PathBuf};

const SUPERVISOR_COMMAND_BOUNDARY: std::time::Duration = std::time::Duration::from_millis(900);
const SUPERVISOR_COMMAND_EXECUTION_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(800);

fn supervisor_command_boundary_error(elapsed: std::time::Duration) -> String {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-supervisor-wall-failure",
        "schemaVersion": "1",
        "state": "unavailable",
        "surface": "runtime-server-supervisor",
        "stage": "runtime-server-reconcile",
        "reasonKind": "runtime-server-supervisor-boundary-exceeded",
        "boundaryMicros": SUPERVISOR_COMMAND_BOUNDARY.as_micros(),
        "executionBudgetMicros": SUPERVISOR_COMMAND_EXECUTION_BUDGET.as_micros(),
        "elapsedMicros": elapsed.as_micros(),
        "retryAfterMs": 250,
    })
    .to_string()
}

#[derive(Debug)]
enum SupervisorCommandError {
    Configuration(String),
    Io(std::io::Error),
    Timeout(std::time::Duration),
}

fn supervisor_command_error(error: SupervisorCommandError, action: &str) -> String {
    match error {
        SupervisorCommandError::Configuration(error) => {
            format!("failed to {action}: {error}")
        }
        SupervisorCommandError::Io(error) => format!("failed to {action}: {error}"),
        SupervisorCommandError::Timeout(elapsed) => supervisor_command_boundary_error(elapsed),
    }
}

struct Command {
    program: Result<std::path::PathBuf, String>,
    args: Vec<std::ffi::OsString>,
}

impl Command {
    fn new(program: impl AsRef<std::ffi::OsStr>) -> Self {
        Self {
            program: resolve_supervisor_command(program.as_ref()),
            args: Vec::new(),
        }
    }

    fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    async fn output(&mut self) -> Result<std::process::Output, SupervisorCommandError> {
        let program = self
            .program
            .as_ref()
            .map_err(|error| SupervisorCommandError::Configuration(error.clone()))?;
        let mut command = tokio::process::Command::new(program);
        command.args(&self.args).kill_on_drop(true);
        let started = std::time::Instant::now();
        match tokio::time::timeout(SUPERVISOR_COMMAND_EXECUTION_BUDGET, command.output()).await {
            Ok(output) => output.map_err(SupervisorCommandError::Io),
            Err(_) => Err(SupervisorCommandError::Timeout(started.elapsed())),
        }
    }
}

fn resolve_supervisor_command(program: &std::ffi::OsStr) -> Result<std::path::PathBuf, String> {
    let program = program
        .to_str()
        .ok_or_else(|| "runtime supervisor command must be valid UTF-8".to_owned())?;
    let (override_key, default_path) = match program {
        "launchctl" | "/bin/launchctl" => ("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/bin/launchctl"),
        "systemctl" | "/usr/bin/systemctl" => {
            ("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/systemctl")
        }
        _ => {
            let path = std::path::PathBuf::from(program);
            if path.is_absolute() {
                return Ok(path);
            }
            return Err(format!(
                "runtime supervisor command must be launchctl, systemctl, or an absolute path: {program}"
            ));
        }
    };
    let path = std::env::var_os(override_key)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(default_path));
    if !path.is_absolute() {
        return Err(format!(
            "{override_key} must name an absolute command path: {}",
            path.display()
        ));
    }
    Ok(path)
}

#[cfg(test)]
#[path = "../../tests/unit/command/runtime_server_supervisor_command.rs"]
mod supervisor_command_contract_tests;

#[cfg(test)]
#[path = "../../tests/unit/command/runtime_server_supervisor_graph_turbo.rs"]
mod supervisor_graph_turbo_environment_tests;

use super::runtime_server_definition::atomic_write_if_changed;
use super::runtime_server_service_catalog::runtime_server_service_catalog;
#[cfg(target_os = "linux")]
const LINUX_SERVICE_NAME: &str = "asp-runtime-server.service";

pub(crate) fn install_runtime_server_supervisor(protocol_home: &Path) -> Result<(), String> {
    super::runtime_server::block_on_runtime_server_client(install(protocol_home))?
}

pub(crate) async fn reconcile_runtime_server_supervisor(
    protocol_home: &Path,
) -> Result<(), String> {
    let runtime_artifact = canonical_supervisor_runtime_artifact(protocol_home).await?;
    #[cfg(target_os = "macos")]
    {
        let target = home_directory()?
            .join("Library")
            .join("LaunchAgents")
            .join(format!(
                "{}.plist",
                runtime_server_service_catalog().active_macos_label
            ));
        reconcile_launchd(&target, &runtime_artifact, false).await
    }
    #[cfg(target_os = "linux")]
    {
        require_command_success(
            Command::new("systemctl")
                .args(["--user", "restart", LINUX_SERVICE_NAME])
                .output()
                .await,
            "reconcile ASP Runtime Server systemd user service",
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = runtime_artifact;
        Err("ASP Runtime Server supervisor is unsupported on this platform".to_owned())
    }
}

/// Removes the Runtime Server from the platform supervisor without addressing
/// an implementation PID directly. The supervisor remains the sole lifecycle
/// authority and a later explicit reconcile may install the latest generation.
pub(crate) async fn stop_runtime_server_supervisor() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let service = format!(
            "gui/{}/{}",
            unsafe { libc::getuid() },
            runtime_server_service_catalog().active_macos_label
        );
        let inspection = Command::new("/bin/launchctl")
            .args(["print", &service])
            .output()
            .await
            .map_err(|error| {
                supervisor_command_error(error, "inspect ASP Runtime Server launchd service")
            })?;
        if !inspection.status.success() {
            return Ok(());
        }
        return require_command_success(
            Command::new("/bin/launchctl")
                .args(["bootout", &service])
                .output()
                .await,
            "stop ASP Runtime Server launchd service",
        );
    }
    #[cfg(target_os = "linux")]
    {
        return require_command_success(
            Command::new("systemctl")
                .args(["--user", "stop", LINUX_SERVICE_NAME])
                .output()
                .await,
            "stop ASP Runtime Server systemd user service",
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("ASP Runtime Server supervisor is unsupported on this platform".to_owned())
}

/// Reconcile the platform supervisor and return only after the latest Runtime
/// Server generation publishes a healthy control-plane receipt.
pub(crate) async fn reconcile_healthy_runtime_server(
    protocol_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    install(protocol_home).await?;
    super::runtime_server::await_healthy_runtime_server(protocol_home).await
}

pub(super) async fn configured_graph_turbo_python_at_state_home(
    state_home: &Path,
    configured: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let config_path = state_home
        .join("runtime")
        .join("server")
        .join("graph-turbo-resident-config.v1.json");
    if let Some(configured) = configured {
        let configured = validate_graph_turbo_python(state_home, configured).await?;
        let document = serde_json::json!({
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-config",
            "schemaVersion": "1",
            "pythonExecutionLocator": configured,
        });
        let parent = config_path
            .parent()
            .ok_or_else(|| "Graph Turbo resident config path has no parent".to_owned())?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        let staged = config_path.with_extension(format!("json.stage-{}", std::process::id()));
        tokio::fs::write(
            &staged,
            serde_json::to_vec_pretty(&document)
                .map_err(|error| format!("encode Graph Turbo resident config: {error}"))?,
        )
        .await
        .map_err(|error| format!("failed to write {}: {error}", staged.display()))?;
        tokio::fs::rename(&staged, &config_path)
            .await
            .map_err(|error| {
                format!(
                    "failed to publish Graph Turbo resident config {}: {error}",
                    config_path.display()
                )
            })?;
        return Ok(Some(configured));
    }
    let document = match tokio::fs::read(&config_path).await {
        Ok(document) => document,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read Graph Turbo resident config {}: {error}",
                config_path.display()
            ));
        }
    };
    let document: serde_json::Value = serde_json::from_slice(&document).map_err(|error| {
        format!(
            "failed to decode Graph Turbo resident config {}: {error}",
            config_path.display()
        )
    })?;
    if document.get("schemaId").and_then(serde_json::Value::as_str)
        != Some("agent.semantic-protocols.semantic-graph-turbo-resident-config")
        || document
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str)
            != Some("1")
    {
        return Err("Graph Turbo resident config schema mismatch".to_owned());
    }
    let configured = document
        .get("pythonExecutionLocator")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "Graph Turbo resident config locator is missing".to_owned())?;
    validate_graph_turbo_python(state_home, configured)
        .await
        .map(Some)
}

async fn validate_graph_turbo_python(
    state_home: &Path,
    configured: PathBuf,
) -> Result<PathBuf, String> {
    if !configured.is_absolute() {
        return Err("ASP_GRAPH_TURBO_PYTHON must be an absolute path".to_owned());
    }
    let canonical = tokio::fs::canonicalize(&configured)
        .await
        .map_err(|error| {
            format!(
                "failed to resolve Graph Turbo Python artifact `{}`: {error}",
                configured.display()
            )
        })?;
    let managed_root = tokio::fs::canonicalize(state_home.join("runtime"))
        .await
        .map_err(|error| format!("failed to resolve managed Runtime root: {error}"))?;
    if !canonical.starts_with(&managed_root) {
        return Err(format!(
            "Graph Turbo runtime artifact is outside the managed Runtime root: artifact={} managedRoot={}",
            canonical.display(),
            managed_root.display()
        ));
    }
    Ok(canonical)
}

async fn install(protocol_home: &Path) -> Result<(), String> {
    let runtime_artifact = canonical_supervisor_runtime_artifact(protocol_home).await?;
    let runtime_reconciliation =
        super::runtime_server::healthcheck_runtime_server_at(protocol_home).await;
    let runtime_is_healthy = matches!(
        &runtime_reconciliation,
        Ok(receipt)
            if receipt.state
                == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
    );

    #[cfg(target_os = "macos")]
    {
        let home = home_directory()?;
        let target = home.join("Library").join("LaunchAgents").join(format!(
            "{}.plist",
            runtime_server_service_catalog().active_macos_label
        ));
        let rendered = include_str!(
            "../../templates/server/dev.tao3k.agent-semantic-protocols.asp-runtime-server.plist"
        )
        .replace("@ASP_RUNTIME@", &runtime_artifact.to_string_lossy())
        .replace("@ASP_STATE_HOME@", &protocol_home.to_string_lossy());
        let definition_changed = atomic_write_if_changed(&target, rendered.as_bytes()).await?;
        if let Ok(receipt) = &runtime_reconciliation
            && server_owned_restart_is_authoritative(definition_changed, receipt.state)
        {
            return Ok(());
        }
        if super::runtime_server_artifact::runtime_server_supervisor_action(
            runtime_is_healthy,
            definition_changed,
        ) == super::runtime_server_artifact::RuntimeServerSupervisorAction::Noop
        {
            return Ok(());
        }
        validate_supervisor_runtime_artifact(&runtime_artifact).await?;
        retire_legacy_launchd(&home).await?;
        reconcile_launchd(&target, &runtime_artifact, definition_changed).await
    }
    #[cfg(target_os = "linux")]
    {
        let target = linux_user_service_directory()?
            .join("systemd")
            .join("user")
            .join(LINUX_SERVICE_NAME);
        let rendered = include_str!("../../templates/server/asp-runtime-server.service")
            .replace("@ASP_RUNTIME@", &runtime_artifact.to_string_lossy())
            .replace("@ASP_STATE_HOME@", &protocol_home.to_string_lossy());
        let definition_changed = atomic_write_if_changed(&target, rendered.as_bytes()).await?;
        if let Ok(receipt) = &runtime_reconciliation
            && server_owned_restart_is_authoritative(definition_changed, receipt.state)
        {
            return Ok(());
        }
        if super::runtime_server_artifact::runtime_server_supervisor_action(
            runtime_is_healthy,
            definition_changed,
        ) == super::runtime_server_artifact::RuntimeServerSupervisorAction::Noop
        {
            return Ok(());
        }
        validate_supervisor_runtime_artifact(&runtime_artifact).await?;
        require_command_success(
            Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output()
                .await,
            "reload ASP Runtime Server systemd user service",
        )?;
        require_command_success(
            Command::new("systemctl")
                .args(["--user", "enable", LINUX_SERVICE_NAME])
                .output()
                .await,
            "enable ASP Runtime Server systemd user service",
        )?;
        require_command_success(
            Command::new("systemctl")
                .args(["--user", "restart", LINUX_SERVICE_NAME])
                .output()
                .await,
            "reconcile ASP Runtime Server systemd user service",
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (protocol_home, runtime_artifact);
        Err("ASP Runtime Server supervisor is unsupported on this platform".to_owned())
    }
}

fn server_owned_restart_is_authoritative(
    supervisor_definition_changed: bool,
    state: agent_semantic_client_db::runtime_server_control::RuntimeServerState,
) -> bool {
    !supervisor_definition_changed
        && matches!(
            state,
            agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
                | agent_semantic_client_db::runtime_server_control::RuntimeServerState::Starting
                | agent_semantic_client_db::runtime_server_control::RuntimeServerState::Draining
        )
}

async fn canonical_supervisor_runtime_artifact(protocol_home: &Path) -> Result<PathBuf, String> {
    let stable_entry = protocol_home.join("runtime").join("bin").join("asp");
    let resolved = tokio::fs::canonicalize(&stable_entry)
        .await
        .map_err(|error| {
            format!(
                "canonical ASP Runtime Server binary is unavailable at {}: {error}",
                stable_entry.display()
            )
        })?;
    let metadata = tokio::fs::metadata(&resolved).await.map_err(|error| {
        format!(
            "canonical ASP Runtime Server binary target is unavailable at {}: {error}",
            resolved.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "canonical ASP Runtime Server binary target is not a file: {}",
            resolved.display()
        ));
    }
    Ok(stable_entry)
}

async fn validate_supervisor_runtime_artifact(runtime_artifact: &Path) -> Result<(), String> {
    crate::command::protocol_binary::canonical_protocol_binary_artifact_digest(&runtime_artifact)
        .await?;
    Ok(())
}

#[cfg(target_os = "macos")]
async fn retire_legacy_launchd(home: &Path) -> Result<(), String> {
    for label in runtime_server_service_catalog().retired_macos_labels {
        let service = format!("gui/{}/{}", unsafe { libc::getuid() }, label);
        let present = Command::new("/bin/launchctl")
            .args(["print", &service])
            .output()
            .await
            .map_err(|error| {
                supervisor_command_error(
                    error,
                    &format!("inspect retired ASP launchd service {label}"),
                )
            })?
            .status
            .success();
        if present {
            require_command_success(
                Command::new("/bin/launchctl")
                    .args(["bootout", &service])
                    .output()
                    .await,
                "retire removed ASP launchd service",
            )?;
        }
        let plist = home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{label}.plist"));
        match tokio::fs::remove_file(&plist).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to remove retired ASP launchd plist at {}: {error}",
                    plist.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaunchdReconcilePlan {
    Bootstrap,
    Kickstart,
    Rebootstrap,
}

#[cfg(target_os = "macos")]
impl LaunchdReconcilePlan {
    fn requires_kickstart(self) -> bool {
        matches!(self, Self::Kickstart)
    }
}

#[cfg(target_os = "macos")]
fn launchd_reconcile_plan(present: bool, definition_changed: bool) -> LaunchdReconcilePlan {
    match (present, definition_changed) {
        (false, _) => LaunchdReconcilePlan::Bootstrap,
        (true, false) => LaunchdReconcilePlan::Kickstart,
        (true, true) => LaunchdReconcilePlan::Rebootstrap,
    }
}

#[cfg(target_os = "macos")]
async fn reconcile_launchd(
    plist: &Path,
    runtime_artifact: &Path,
    definition_changed: bool,
) -> Result<(), String> {
    let service = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() },
        runtime_server_service_catalog().active_macos_label
    );
    let inspection = Command::new("/bin/launchctl")
        .args(["print", &service])
        .output()
        .await
        .map_err(|error| {
            supervisor_command_error(error, "inspect ASP Runtime Server launchd service")
        })?;
    let present = inspection.status.success();
    let loaded_definition_matches =
        present && launchd_loaded_program_matches(&inspection.stdout, runtime_artifact);
    let plan = launchd_reconcile_plan(
        present,
        definition_changed || (present && !loaded_definition_matches),
    );
    match plan {
        LaunchdReconcilePlan::Kickstart => {}
        LaunchdReconcilePlan::Bootstrap => {
            bootstrap_launchd(plist).await?;
        }
        LaunchdReconcilePlan::Rebootstrap => {
            require_command_success(
                Command::new("/bin/launchctl")
                    .args([
                        "bootout",
                        &format!("gui/{}", unsafe { libc::getuid() }),
                        &plist.to_string_lossy(),
                    ])
                    .output()
                    .await,
                "remove stale ASP Runtime Server launchd definition",
            )?;
            bootstrap_launchd(plist).await?;
        }
    }
    if !plan.requires_kickstart() {
        return Ok(());
    }
    require_command_success(
        Command::new("/bin/launchctl")
            .args(["kickstart", "-k", &service])
            .output()
            .await,
        "reconcile ASP Runtime Server launchd service",
    )
}

#[cfg(target_os = "macos")]
fn launchd_loaded_program_matches(output: &[u8], runtime_artifact: &Path) -> bool {
    let Ok(output) = std::str::from_utf8(output) else {
        return false;
    };
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("program = ")
            .map(std::path::Path::new)
    }) == Some(runtime_artifact)
}

#[cfg(target_os = "macos")]
async fn bootstrap_launchd(plist: &Path) -> Result<(), String> {
    require_command_success(
        Command::new("/bin/launchctl")
            .args([
                "bootstrap",
                &format!("gui/{}", unsafe { libc::getuid() }),
                &plist.to_string_lossy(),
            ])
            .output()
            .await,
        "bootstrap ASP Runtime Server launchd service",
    )
}

fn require_command_success(
    output: Result<std::process::Output, SupervisorCommandError>,
    action: &str,
) -> Result<(), String> {
    let output = output.map_err(|error| supervisor_command_error(error, action))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "failed to {action}: status={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn home_directory() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is unset; cannot install Global ASP launchd service".to_owned())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "../../tests/unit/runtime_server_supervisor.rs"]
mod runtime_server_supervisor_tests;

#[cfg(target_os = "linux")]
fn linux_user_service_directory() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    Ok(home_directory()?.join(".config"))
}
