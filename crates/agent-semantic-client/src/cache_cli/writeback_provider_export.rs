//! Provider packet export for write-back side artifacts.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use agent_semantic_client_core::{
    ByteCount, ClientMethod, ClientRequest, ElapsedMillis, ProviderCommandReceipt,
    ResolvedProvider, append_syntax_query_plan_args,
};
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode,
    run_provider_process as run_transport_process,
};
use bytes::Bytes;

use super::writeback_request::insert_json_flag_before_project_root;
use crate::cache_replay::MAX_CACHE_REPLAY_ARTIFACT_BYTES;

pub(super) struct ProviderPacketExport {
    pub(super) packet_bytes: Bytes,
    pub(super) command: ProviderCommandReceipt,
    pub(super) elapsed_ms: ElapsedMillis,
}

pub(super) fn export_provider_packet(
    provider: &ResolvedProvider,
    request: &ClientRequest,
) -> Option<ProviderPacketExport> {
    let program = home_local_provider_binary(provider)?;
    let mut args = Vec::new();
    let provider_method = match request.method {
        ClientMethod::Search => "search",
        ClientMethod::Query => "query",
        _ => return None,
    };
    args.push(provider_method.to_string());
    let mut forwarded_args = append_syntax_query_plan_args(
        &request.method,
        Some(&provider.language_id),
        request.forwarded_args.clone(),
    )
    .ok()?;
    insert_json_flag_before_project_root(&mut forwarded_args);
    args.extend(forwarded_args);
    let argv = std::iter::once(program.clone())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    let started = Instant::now();
    let output = run_transport_process(ProviderProcessSpec {
        program: program.clone(),
        args,
        cwd: request.project_root.clone(),
        env: BTreeMap::new(),
        stdin: StdinMode::Closed,
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    })
    .ok()?;
    if !output.status.success()
        || output.stdout.is_empty()
        || output.receipt.stdout_bytes as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES
    {
        return None;
    }
    Some(ProviderPacketExport {
        packet_bytes: output.stdout,
        command: ProviderCommandReceipt {
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
            argv,
            exit_code: output.status.code().unwrap_or(1),
            stdout_bytes: ByteCount::from_len(output.receipt.stdout_bytes),
            stderr_bytes: ByteCount::from_len(output.receipt.stderr_bytes),
            stdout_sha256: output.receipt.stdout_sha256.clone(),
            stderr_sha256: output.receipt.stderr_sha256.clone(),
            stdout_truncated: output.receipt.stdout_truncated,
            stderr_truncated: output.receipt.stderr_truncated,
            timed_out: output.receipt.timed_out,
            elapsed_ms: ElapsedMillis::from_duration(output.receipt.elapsed),
        },
        elapsed_ms: ElapsedMillis::from_duration(started.elapsed()),
    })
}

fn home_local_provider_binary(provider: &ResolvedProvider) -> Option<String> {
    let home = std::env::var_os("HOME").filter(|value| !value.is_empty())?;
    let path = Path::new(&home).join(".local/bin").join(&provider.binary);
    path.is_file().then(|| path.to_string_lossy().to_string())
}
