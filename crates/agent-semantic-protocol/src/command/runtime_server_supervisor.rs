use std::path::{Path, PathBuf};

use tokio::process::Command;

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
    install(protocol_home).await
}

/// Reconcile the platform supervisor and return only after the latest Runtime
/// Server generation publishes a healthy control-plane receipt.
pub(crate) async fn reconcile_healthy_runtime_server(
    protocol_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    install(protocol_home).await?;
    super::runtime_server::await_healthy_runtime_server(protocol_home).await
}

async fn install(protocol_home: &Path) -> Result<(), String> {
    let runtime_artifact = canonical_supervisor_runtime_artifact(protocol_home).await?;
    let runtime_is_healthy = matches!(
        super::runtime_server::healthcheck_runtime_server_at(protocol_home).await,
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
        if super::runtime_server_artifact::runtime_server_supervisor_action(
            runtime_is_healthy,
            definition_changed,
        ) == super::runtime_server_artifact::RuntimeServerSupervisorAction::Noop
        {
            return Ok(());
        }
        retire_legacy_launchd(&home).await?;
        reconcile_launchd(&target, definition_changed).await
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
        if super::runtime_server_artifact::runtime_server_supervisor_action(
            runtime_is_healthy,
            definition_changed,
        ) == super::runtime_server_artifact::RuntimeServerSupervisorAction::Noop
        {
            return Ok(());
        }
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

async fn canonical_supervisor_runtime_artifact(protocol_home: &Path) -> Result<PathBuf, String> {
    let stable_entry = protocol_home.join("runtime").join("bin").join("asp");
    let runtime_artifact = tokio::fs::canonicalize(&stable_entry)
        .await
        .map_err(|error| {
            format!(
                "canonical ASP Runtime Server binary is unavailable at {}: {error}",
                stable_entry.display()
            )
        })?;
    super::protocol_binary::canonical_protocol_binary_artifact_digest(&runtime_artifact).await?;
    Ok(runtime_artifact)
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
                format!("failed to inspect retired ASP launchd service {label}: {error}")
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
async fn reconcile_launchd(plist: &Path, definition_changed: bool) -> Result<(), String> {
    let service = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() },
        runtime_server_service_catalog().active_macos_label
    );
    let present = Command::new("/bin/launchctl")
        .args(["print", &service])
        .output()
        .await
        .map_err(|error| format!("failed to inspect ASP Runtime Server launchd service: {error}"))?
        .status
        .success();
    let plan = launchd_reconcile_plan(present, definition_changed);
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
    output: Result<std::process::Output, std::io::Error>,
    action: &str,
) -> Result<(), String> {
    let output = output.map_err(|error| format!("failed to {action}: {error}"))?;
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
