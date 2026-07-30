use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const MACOS_SERVICE_LABEL: &str = "dev.tao3k.agent-semantic-protocols.asp-resident";
const LINUX_SERVICE_NAME: &str = "asp-resident.service";

pub(crate) fn install_global_resident_supervisor(protocol_home: &Path) -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create resident installer Tokio runtime: {error}"))?
        .block_on(install(protocol_home))
}

pub(crate) async fn reconcile_global_resident_supervisor() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let service = format!("gui/{}/{}", unsafe { libc::getuid() }, MACOS_SERVICE_LABEL);
        return require_command_success(
            Command::new("/bin/launchctl")
                .args(["kickstart", "-k", &service])
                .output()
                .await,
            "reconcile Global ASP launchd service",
        );
    }
    #[cfg(target_os = "linux")]
    {
        return require_command_success(
            Command::new("systemctl")
                .args(["--user", "restart", LINUX_SERVICE_NAME])
                .output()
                .await,
            "reconcile Global ASP systemd user service",
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("Global ASP resident supervisor is unsupported on this platform".to_owned())
}

async fn install(protocol_home: &Path) -> Result<(), String> {
    let runtime_artifact = protocol_home.join("runtime").join("bin").join("asp");
    tokio::fs::metadata(&runtime_artifact)
        .await
        .map_err(|error| {
            format!(
                "canonical Global ASP runtime is unavailable at {}: {error}",
                runtime_artifact.display()
            )
        })?;

    #[cfg(target_os = "macos")]
    {
        let home = home_directory()?;
        let target = home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{MACOS_SERVICE_LABEL}.plist"));
        let rendered = include_str!(
            "../../templates/resident/dev.tao3k.agent-semantic-protocols.asp-resident.plist"
        )
        .replace("@ASP_RUNTIME@", &runtime_artifact.to_string_lossy())
        .replace("@ASP_STATE_HOME@", &protocol_home.to_string_lossy());
        atomic_write(&target, rendered.as_bytes()).await?;
        reconcile_launchd(&target).await
    }
    #[cfg(target_os = "linux")]
    {
        let target = linux_user_service_directory()?
            .join("systemd")
            .join("user")
            .join(LINUX_SERVICE_NAME);
        let rendered = include_str!("../../templates/resident/asp-resident.service")
            .replace("@ASP_RUNTIME@", &runtime_artifact.to_string_lossy())
            .replace("@ASP_STATE_HOME@", &protocol_home.to_string_lossy());
        atomic_write(&target, rendered.as_bytes()).await?;
        require_command_success(
            Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output()
                .await,
            "reload Global ASP systemd user service",
        )?;
        require_command_success(
            Command::new("systemctl")
                .args(["--user", "enable", "--now", LINUX_SERVICE_NAME])
                .output()
                .await,
            "enable Global ASP systemd user service",
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (protocol_home, runtime_artifact);
        Err("Global ASP resident supervisor is unsupported on this platform".to_owned())
    }
}

#[cfg(target_os = "macos")]
async fn reconcile_launchd(plist: &Path) -> Result<(), String> {
    let service = format!("gui/{}/{}", unsafe { libc::getuid() }, MACOS_SERVICE_LABEL);
    let present = Command::new("/bin/launchctl")
        .args(["print", &service])
        .output()
        .await
        .map_err(|error| format!("failed to inspect Global ASP launchd service: {error}"))?
        .status
        .success();
    if !present {
        require_command_success(
            Command::new("/bin/launchctl")
                .args([
                    "bootstrap",
                    &format!("gui/{}", unsafe { libc::getuid() }),
                    &plist.to_string_lossy(),
                ])
                .output()
                .await,
            "bootstrap Global ASP launchd service",
        )?;
    }
    require_command_success(
        Command::new("/bin/launchctl")
            .args(["kickstart", "-k", &service])
            .output()
            .await,
        "reconcile Global ASP launchd service",
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

async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "resident supervisor path has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        format!(
            "failed to create resident supervisor directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = path.with_extension(format!("tmp-{}", entropy_digest().await?));
    tokio::fs::write(&temporary, bytes).await.map_err(|error| {
        format!(
            "failed to write resident supervisor temporary file {}: {error}",
            temporary.display()
        )
    })?;
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        format!(
            "failed to publish resident supervisor definition {}: {error}",
            path.display()
        )
    })
}

async fn entropy_digest() -> Result<String, String> {
    let mut source = tokio::fs::File::open("/dev/urandom")
        .await
        .map_err(|error| format!("failed to open OS entropy source: {error}"))?;
    let mut entropy = [0_u8; 16];
    source
        .read_exact(&mut entropy)
        .await
        .map_err(|error| format!("failed to read OS entropy: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(entropy)))
}

fn home_directory() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is unset; cannot install Global ASP launchd service".to_owned())
}

#[cfg(target_os = "linux")]
fn linux_user_service_directory() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    Ok(home_directory()?.join(".config"))
}
