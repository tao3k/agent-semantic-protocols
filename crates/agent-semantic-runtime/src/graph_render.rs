//! Runtime transport for replay-time graph rendering.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, ProviderProcessSupervisor, StdinMode,
};
use bytes::Bytes;

const SEMANTIC_AGENT_PROTOCOL_BIN_ENV: &str = "SEMANTIC_AGENT_PROTOCOL_BIN";

pub struct GraphRenderReceiptRequest {
    pub out_path: PathBuf,
    pub receipt_id: String,
    pub task_fingerprint: String,
    pub command_fingerprint: String,
}

pub async fn run_graph_render_packet(
    supervisor: &ProviderProcessSupervisor,
    packet_path: &Path,
    max_stdout_bytes: u64,
) -> Option<Bytes> {
    run_graph_render_process(
        supervisor,
        packet_path.display().to_string(),
        StdinMode::Closed,
        max_stdout_bytes,
        None,
        false,
    )
    .await
    .ok()
    .flatten()
}

pub async fn run_graph_render_packet_bytes(
    supervisor: &ProviderProcessSupervisor,
    packet_bytes: impl Into<Bytes>,
    max_stdout_bytes: u64,
) -> Option<Bytes> {
    run_graph_render_process(
        supervisor,
        "-".to_string(),
        StdinMode::bytes(packet_bytes.into()),
        max_stdout_bytes,
        None,
        false,
    )
    .await
    .ok()
    .flatten()
}

pub async fn run_graph_render_packet_bytes_with_receipt(
    supervisor: &ProviderProcessSupervisor,
    packet_bytes: impl Into<Bytes>,
    max_stdout_bytes: u64,
    receipt: &GraphRenderReceiptRequest,
) -> Result<Option<Bytes>, String> {
    run_graph_render_process(
        supervisor,
        "-".to_string(),
        StdinMode::bytes(packet_bytes.into()),
        max_stdout_bytes,
        Some(receipt),
        true,
    )
    .await
}

async fn run_graph_render_process(
    supervisor: &ProviderProcessSupervisor,
    packet_arg: String,
    stdin: StdinMode,
    max_stdout_bytes: u64,
    receipt: Option<&GraphRenderReceiptRequest>,
    strict: bool,
) -> Result<Option<Bytes>, String> {
    let mut args = vec![
        "graph".to_string(),
        "render".to_string(),
        "--packet".to_string(),
        packet_arg,
        "--view".to_string(),
        "seeds".to_string(),
    ];
    if let Some(receipt) = receipt {
        args.extend([
            "--frontier-receipt-out".to_string(),
            receipt.out_path.display().to_string(),
            "--receipt-id".to_string(),
            receipt.receipt_id.clone(),
            "--task-fingerprint".to_string(),
            receipt.task_fingerprint.clone(),
            "--command-fingerprint".to_string(),
            receipt.command_fingerprint.clone(),
        ]);
    }
    let output = match supervisor
        .run(ProviderProcessSpec {
            program: protocol_graph_renderer_binary().display().to_string(),
            args,
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env: BTreeMap::new(),
            remove_env: Default::default(),
            remove_env_prefixes: Default::default(),
            stdin,
            stdout: OutputMode::Capture,
            stderr: OutputMode::Capture,
            limits: ProviderProcessLimits::new(
                None,
                Some(max_stdout_bytes as usize + 1),
                Some(64 * 1024),
                Some(1024 * 1024 * 1024),
            ),
        })
        .await
    {
        Ok(output) => output,
        Err(error) if strict => return Err(format!("failed to run graph renderer: {error}")),
        Err(_) => return Ok(None),
    };
    if !output.status.success()
        || output.stdout.is_empty()
        || output.receipt.stdout_bytes() as u64 > max_stdout_bytes
    {
        if strict {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "graph renderer failed while writing frontier receipt: status={} stderr={}",
                output.status.code().unwrap_or(1),
                stderr.trim()
            ));
        }
        return Ok(None);
    }
    Ok(Some(output.stdout))
}

fn protocol_graph_renderer_binary() -> PathBuf {
    env::var_os(SEMANTIC_AGENT_PROTOCOL_BIN_ENV)
        .map(PathBuf::from)
        .or_else(|| env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("asp"))
}
