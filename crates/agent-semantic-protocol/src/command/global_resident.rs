use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_semantic_client_db::global_resident_control::{
    read_global_resident_request, write_global_resident_receipt,
};
use agent_semantic_client_db::{
    GlobalResidentControlReceipt, GlobalResidentEndpoint, GlobalResidentOperation,
    WorkspaceDbRegistry, acquire_global_resident_election, call_global_resident,
    global_resident_endpoint_path, prepare_global_resident_endpoint,
};
use clap::{Parser, Subcommand};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnixListenerStream;

#[derive(Debug, Parser)]
#[command(name = "asp resident", disable_help_subcommand = true)]
struct ResidentArgs {
    #[command(subcommand)]
    command: ResidentCommand,
}

#[derive(Debug, Subcommand)]
enum ResidentCommand {
    Status,
    Reconcile,
    Restart,
    /// Run the long-lived Global ASP daemon under the platform supervisor.
    Daemon,
}

pub(crate) fn run_global_resident_command(args: &[String]) -> Result<(), String> {
    let parsed = ResidentArgs::try_parse_from(
        std::iter::once("asp resident".to_owned()).chain(args.iter().cloned()),
    )
    .map_err(|error| error.to_string())?;
    match parsed.command {
        ResidentCommand::Daemon => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to create Global ASP daemon Tokio runtime: {error}"))?
            .block_on(run_daemon()),
        command => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to create resident client Tokio runtime: {error}"))?
            .block_on(async move {
                match command {
                    ResidentCommand::Status => run_control(GlobalResidentOperation::Status).await,
                    ResidentCommand::Reconcile => {
                        run_control(GlobalResidentOperation::Reconcile).await
                    }
                    ResidentCommand::Restart => run_control(GlobalResidentOperation::Restart).await,
                    ResidentCommand::Daemon => unreachable!("daemon handled before client runtime"),
                }
            }),
    }
}

async fn run_control(operation: GlobalResidentOperation) -> Result<(), String> {
    let state_home = state_home()?;
    let endpoint_path = global_resident_endpoint_path(&state_home);
    let endpoint = read_endpoint(&endpoint_path).await;
    let request_id = request_identity("control").await?;
    let endpoint = match endpoint {
        Ok(endpoint) => endpoint,
        Err(error)
            if matches!(
                operation,
                GlobalResidentOperation::Reconcile | GlobalResidentOperation::Restart
            ) =>
        {
            super::resident_supervisor::reconcile_global_resident_supervisor().await?;
            let runtime_artifact_path = state_home.join("runtime").join("bin").join("asp");
            let runtime_artifact_digest = digest_file(&runtime_artifact_path).await?;
            let receipt = GlobalResidentControlReceipt::starting(
                request_id,
                runtime_artifact_digest,
                format!("platform supervisor reconcile requested: {error}"),
            );
            print_receipt(&receipt).await?;
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let receipt = call_global_resident(&endpoint, operation, request_id).await?;
    print_receipt(&receipt).await
}

async fn print_receipt(receipt: &GlobalResidentControlReceipt) -> Result<(), String> {
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
    let election = acquire_global_resident_election().await?;
    let state_home = state_home()?;
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_artifact_digest = digest_file(&runtime_artifact_path).await?;
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let endpoint = prepare_global_resident_endpoint(
        &runtime_artifact_path,
        &runtime_artifact_digest,
        owner_epoch,
        &binding_token,
    )
    .await?;
    let socket_path = PathBuf::from(&endpoint.socket_path);
    remove_stale_socket(&socket_path).await?;
    let listener = UnixListener::bind(&socket_path).map_err(|error| {
        format!(
            "failed to bind global resident socket {}: {error}",
            socket_path.display()
        )
    })?;
    publish_endpoint(&state_home, &endpoint).await?;

    let registry = Arc::new(WorkspaceDbRegistry::default());
    let result = serve_control_loop(listener, endpoint.clone(), registry).await;
    cleanup_endpoint(&state_home, &endpoint).await;
    drop(election);
    result
}

async fn serve_control_loop(
    listener: UnixListener,
    endpoint: GlobalResidentEndpoint,
    registry: Arc<WorkspaceDbRegistry>,
) -> Result<(), String> {
    let mut incoming = UnixListenerStream::new(listener);
    let mut connections = JoinSet::new();
    let (drain_sender, drain_receiver) = watch::channel(false);
    loop {
        tokio::select! {
            connection = incoming.next() => {
                let Some(connection) = connection else {
                    break;
                };
                let stream = connection
                    .map_err(|error| format!("failed to accept global resident request: {error}"))?;
                connections.spawn(serve_control_connection(
                    stream,
                    endpoint.clone(),
                    Arc::clone(&registry),
                    drain_receiver.clone(),
                ));
            }
            completed = connections.join_next(), if !connections.is_empty() => {
                match completed {
                    Some(Ok(Ok(true))) => {
                        let _ = drain_sender.send(true);
                        break;
                    }
                    Some(Ok(Ok(false))) => {}
                    Some(Ok(Err(error))) => {
                        report_daemon_error(format!(
                            "global resident rejected control connection: {error}"
                        ))
                        .await;
                    }
                    Some(Err(error)) => {
                        report_daemon_error(format!(
                            "global resident control task failed: {error}"
                        ))
                        .await;
                    }
                    None => {}
                }
            }
        }
    }
    let _ = drain_sender.send(true);
    drop(incoming);
    while let Some(completed) = connections.join_next().await {
        match completed {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                report_daemon_error(format!(
                    "global resident rejected control connection during drain: {error}"
                ))
                .await;
            }
            Err(error) => {
                report_daemon_error(format!(
                    "global resident control task failed during drain: {error}"
                ))
                .await;
            }
        }
    }
    Ok(())
}

async fn serve_control_connection(
    mut stream: UnixStream,
    endpoint: GlobalResidentEndpoint,
    registry: Arc<WorkspaceDbRegistry>,
    mut drain: watch::Receiver<bool>,
) -> Result<bool, String> {
    let request = tokio::select! {
        request = read_global_resident_request(&mut stream) => request?,
        changed = drain.changed() => {
            let _ = changed;
            return Ok(false);
        }
    };
    request.validate_for_endpoint(&endpoint)?;
    let restart = request.operation == GlobalResidentOperation::Restart;
    let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
    let workspace_entry_count = slot_count.max(loaded_entry_count);
    let receipt = if restart {
        GlobalResidentControlReceipt::draining(request.request_id, &endpoint, workspace_entry_count)
    } else {
        GlobalResidentControlReceipt::healthy(request.request_id, &endpoint, workspace_entry_count)
    };
    write_global_resident_receipt(&mut stream, &receipt).await?;
    Ok(restart)
}

async fn report_daemon_error(message: String) {
    let mut stderr = tokio::io::stderr();
    let _ = stderr.write_all(message.as_bytes()).await;
    let _ = stderr.write_all(b"\n").await;
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

async fn read_endpoint(path: &Path) -> Result<GlobalResidentEndpoint, String> {
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        format!(
            "global resident endpoint is unavailable at {}: {error}",
            path.display()
        )
    })?;
    let endpoint: GlobalResidentEndpoint = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode global resident endpoint: {error}"))?;
    endpoint.validate()?;
    Ok(endpoint)
}

async fn publish_endpoint(
    state_home: &Path,
    endpoint: &GlobalResidentEndpoint,
) -> Result<(), String> {
    atomic_write_json(&global_resident_endpoint_path(state_home), endpoint).await
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
            "failed to remove stale global resident socket {}: {error}",
            path.display()
        )),
    }
}

async fn cleanup_endpoint(state_home: &Path, endpoint: &GlobalResidentEndpoint) {
    let endpoint_path = global_resident_endpoint_path(state_home);
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let owned = read_endpoint(&endpoint_path).await.is_ok_and(|actual| {
        actual.owner_epoch == endpoint.owner_epoch && actual.binding_token == endpoint.binding_token
    });
    if owned {
        let _ = tokio::fs::remove_file(endpoint_path).await;
        let _ = tokio::fs::remove_file(socket_path).await;
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
