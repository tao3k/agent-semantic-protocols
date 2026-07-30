use std::path::{Path, PathBuf};
use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerOperation, WorkspaceDbRegistry,
    acquire_runtime_server_election, call_runtime_server, prepare_runtime_server_endpoint,
    runtime_server_endpoint_path,
};
use clap::{Parser, Subcommand};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Parser)]
#[command(name = "asp server", disable_help_subcommand = true)]
struct ServerArgs {
    #[command(subcommand)]
    command: ServerCommand,
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    Status,
    Reconcile,
    Restart,
    /// Run the long-lived Global ASP daemon under the platform supervisor.
    Daemon,
}

pub(crate) fn run_runtime_server_command(args: &[String]) -> Result<(), String> {
    let parsed = ServerArgs::try_parse_from(
        std::iter::once("asp server".to_owned()).chain(args.iter().cloned()),
    )
    .map_err(|error| error.to_string())?;
    match parsed.command {
        ServerCommand::Daemon => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to create ASP Runtime Server Tokio runtime: {error}"))?
            .block_on(run_daemon()),
        command => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to create ASP Runtime Server client Tokio runtime: {error}"))?
            .block_on(async move {
                match command {
                    ServerCommand::Status => run_control(RuntimeServerOperation::Status).await,
                    ServerCommand::Reconcile => {
                        run_control(RuntimeServerOperation::Reconcile).await
                    }
                    ServerCommand::Restart => run_control(RuntimeServerOperation::Restart).await,
                    ServerCommand::Daemon => unreachable!("daemon handled before client runtime"),
                }
            }),
    }
}

async fn run_control(operation: RuntimeServerOperation) -> Result<(), String> {
    let state_home = state_home()?;
    let endpoint_path = runtime_server_endpoint_path(&state_home);
    let endpoint = read_endpoint(&endpoint_path).await;
    let request_id = request_identity("control").await?;
    let endpoint = match endpoint {
        Ok(endpoint) => endpoint,
        Err(error)
            if matches!(
                operation,
                RuntimeServerOperation::Reconcile | RuntimeServerOperation::Restart
            ) =>
        {
            super::runtime_server_supervisor::reconcile_runtime_server_supervisor().await?;
            let runtime_artifact_path = state_home.join("runtime").join("bin").join("asp");
            let runtime_artifact_digest = digest_file(&runtime_artifact_path).await?;
            let receipt = RuntimeServerControlReceipt::starting(
                request_id,
                runtime_artifact_digest,
                format!("platform supervisor reconcile requested: {error}"),
            );
            print_receipt(&receipt).await?;
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let expected_runtime_artifact_digest = if operation == RuntimeServerOperation::Status {
        endpoint.runtime_artifact_digest.clone()
    } else {
        digest_file(&state_home.join("runtime").join("bin").join("asp")).await?
    };
    let receipt = call_runtime_server(
        &endpoint,
        operation,
        expected_runtime_artifact_digest,
        request_id,
    )
    .await?;
    print_receipt(&receipt).await
}

pub(super) fn healthcheck_runtime_server() -> Result<RuntimeServerControlReceipt, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create resident health Tokio runtime: {error}"))?
        .block_on(healthcheck_runtime_server_async())
}

async fn healthcheck_runtime_server_async() -> Result<RuntimeServerControlReceipt, String> {
    let state_home = state_home()?;
    let endpoint = read_endpoint(&runtime_server_endpoint_path(&state_home)).await?;
    let canonical_runtime = state_home.join("runtime").join("bin").join("asp");
    let canonical_artifact = tokio::fs::canonicalize(&canonical_runtime)
        .await
        .map_err(|error| {
            format!(
                "failed to resolve canonical Global ASP runtime {}: {error}",
                canonical_runtime.display()
            )
        })?;
    let running_artifact = tokio::fs::canonicalize(&endpoint.runtime_artifact_path)
        .await
        .map_err(|error| {
            format!(
                "failed to resolve running Global ASP artifact {}: {error}",
                endpoint.runtime_artifact_path
            )
        })?;
    let (operation, expected_runtime_artifact_digest) =
        if canonical_artifact == running_artifact {
            (
                RuntimeServerOperation::Status,
                endpoint.runtime_artifact_digest.clone(),
            )
        } else {
            (
                RuntimeServerOperation::Restart,
                digest_file(&canonical_runtime).await?,
            )
        };
    call_runtime_server(
        &endpoint,
        operation,
        expected_runtime_artifact_digest,
        request_identity("healthcheck").await?,
    )
    .await
}

async fn print_receipt(receipt: &RuntimeServerControlReceipt) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(receipt)
        .map_err(|error| format!("failed to encode global resident receipt: {error}"))?;
    bytes.push(b'\n');
    let mut stdout = tokio::io::stdout();
    stdout
        .write_all(&bytes)
        .await
        .map_err(|error| format!("failed to write global resident receipt: {error}"))?;
    stdout
        .flush()
        .await
        .map_err(|error| format!("failed to flush global resident receipt: {error}"))?;
    Ok(())
}

async fn run_daemon() -> Result<(), String> {
    let election = acquire_runtime_server_election().await?;
    let state_home = state_home()?;
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_artifact_digest = digest_file(&runtime_artifact_path).await?;
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let endpoint = prepare_runtime_server_endpoint(
        &runtime_artifact_path,
        &runtime_artifact_digest,
        owner_epoch,
        &binding_token,
    )
    .await?;
    let socket_path = PathBuf::from(&endpoint.socket_path);
    remove_stale_socket(&socket_path).await?;
    publish_endpoint(&state_home, &endpoint).await?;

    let server = RuntimeServer::bind(
        endpoint.clone(),
        std::sync::Arc::new(WorkspaceDbRegistry::default()),
    )
    .await?;
    let result = server.serve().await.map(|_| ());
    cleanup_endpoint(&state_home, &endpoint).await;
    drop(election);
    result
}

fn state_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("ASP_STATE_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "ASP_STATE_HOME and HOME are both unset".to_owned())?;
    Ok(PathBuf::from(home).join(".agent-semantic-protocols"))
}

async fn read_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        format!(
            "global resident endpoint is unavailable at {}: {error}",
            path.display()
        )
    })?;
    let endpoint: RuntimeServerEndpoint = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode global resident endpoint: {error}"))?;
    endpoint.validate()?;
    Ok(endpoint)
}

async fn publish_endpoint(
    state_home: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    atomic_write_json(&runtime_server_endpoint_path(state_home), endpoint).await
}

async fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "global resident endpoint path has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        format!(
            "failed to create global resident endpoint directory {}: {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode global resident endpoint: {error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", request_identity("publish").await?));
    tokio::fs::write(&temporary, bytes).await.map_err(|error| {
        format!(
            "failed to write global resident endpoint temporary file {}: {error}",
            temporary.display()
        )
    })?;
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        format!(
            "failed to publish global resident endpoint {}: {error}",
            path.display()
        )
    })
}

async fn remove_stale_socket(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale Runtime Server socket {}: {error}",
            path.display()
        )),
    }
}

async fn cleanup_endpoint(state_home: &Path, endpoint: &RuntimeServerEndpoint) {
    let endpoint_path = runtime_server_endpoint_path(state_home);
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let status_memory_path = PathBuf::from(&endpoint.status_memory_path);
    let owned = read_endpoint(&endpoint_path).await.is_ok_and(|actual| {
        actual.owner_epoch == endpoint.owner_epoch && actual.binding_token == endpoint.binding_token
    });
    if owned {
        let _ = tokio::fs::remove_file(endpoint_path).await;
        let _ = tokio::fs::remove_file(socket_path).await;
        let _ = tokio::fs::remove_file(status_memory_path).await;
    }
}

async fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path).await.map_err(|error| {
        format!(
            "failed to open ASP runtime artifact {}: {error}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await.map_err(|error| {
            format!(
                "failed to read ASP runtime artifact {}: {error}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

async fn request_identity(seed: &str) -> Result<String, String> {
    let entropy = os_entropy().await?;
    Ok(format!(
        "{:x}",
        Sha256::digest([entropy.as_slice(), seed.as_bytes()].concat())
    ))
}

async fn daemon_identity() -> Result<(u64, String), String> {
    let entropy = os_entropy().await?;
    let mut epoch_bytes = [0_u8; 8];
    epoch_bytes.copy_from_slice(&entropy[..8]);
    let owner_epoch = u64::from_le_bytes(epoch_bytes).max(1);
    Ok((owner_epoch, format!("{:x}", Sha256::digest(entropy))))
}

async fn os_entropy() -> Result<[u8; 32], String> {
    let mut source = tokio::fs::File::open("/dev/urandom")
        .await
        .map_err(|error| format!("failed to open OS entropy source: {error}"))?;
    let mut entropy = [0_u8; 32];
    source
        .read_exact(&mut entropy)
        .await
        .map_err(|error| format!("failed to read OS entropy: {error}"))?;
    Ok(entropy)
}
