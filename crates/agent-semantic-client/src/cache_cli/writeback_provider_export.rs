//! Provider packet export for write-back side artifacts.

use std::collections::BTreeMap;
use std::time::Instant;

use agent_semantic_client_core::{
    ByteCount, ClientMethod, ClientRequest, ElapsedMillis, ProviderCommandReceipt,
    ResolvedProvider, append_syntax_query_plan_args,
};
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode,
    run_provider_process_async as run_transport_process,
};
use bytes::Bytes;

use super::writeback_request::insert_json_flag_before_project_root;
use crate::cache_replay::MAX_CACHE_REPLAY_ARTIFACT_BYTES;

pub(super) struct ProviderPacketExport {
    pub(super) packet_bytes: Bytes,
    pub(super) command: ProviderCommandReceipt,
    pub(super) elapsed_ms: ElapsedMillis,
}

pub(super) async fn export_provider_packet(
    provider: &ResolvedProvider,
    request: &ClientRequest,
) -> Option<ProviderPacketExport> {
    let mut invocation = provider_command_prefix(provider)?;
    let provider_method = match request.method {
        ClientMethod::Search => "search",
        ClientMethod::Query => "query",
        _ => return None,
    };
    invocation.push(provider_method.to_string());
    let mut forwarded_args = append_syntax_query_plan_args(
        &request.method,
        Some(&provider.language_id),
        request.forwarded_args.clone(),
    )
    .ok()?;
    insert_json_flag_before_project_root(&mut forwarded_args);
    invocation.extend(forwarded_args);
    let (program, args) = invocation.split_first()?;
    let argv = invocation.clone();
    let started = Instant::now();
    let output = run_transport_process(ProviderProcessSpec {
        program: program.clone(),
        args: args.to_vec(),
        cwd: request.project_root.clone(),
        env: BTreeMap::new(),
        stdin: StdinMode::Closed,
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    })
    .await
    .ok()?;
    if !output.status.success()
        || output.stdout.is_empty()
        || output.receipt.stdout_bytes() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES
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
            stdout_bytes: ByteCount::from_len(output.receipt.stdout_bytes()),
            stderr_bytes: ByteCount::from_len(output.receipt.stderr_bytes()),
            stdout_sha256: output.receipt.stdout_sha256().map(str::to_owned),
            stderr_sha256: output.receipt.stderr_sha256().map(str::to_owned),
            stdout_truncated: output.receipt.stdout_truncated(),
            stderr_truncated: output.receipt.stderr_truncated(),
            timed_out: output.receipt.timed_out(),
            exit_signal: output.receipt.exit_signal(),
            memory_limit_bytes: output.receipt.memory_limit_bytes(),
            memory_limit_enforced: output.receipt.memory_limit_enforced(),
            memory_limit_exceeded: output.receipt.memory_limit_exceeded(),
            abnormal_termination: output.receipt.abnormal_termination(),
            termination_reason: Some(output.receipt.termination_reason().to_string()),
            elapsed_ms: ElapsedMillis::from_duration(output.receipt.elapsed()),
        },
        elapsed_ms: ElapsedMillis::from_duration(started.elapsed()),
    })
}

fn provider_command_prefix(provider: &ResolvedProvider) -> Option<Vec<String>> {
    if let Some(argv) = provider.runtime_command_argv.as_ref()
        && !argv.is_empty()
    {
        return Some(argv.clone());
    }
    if !provider.provider_command_prefix.is_empty() {
        return Some(provider.provider_command_prefix.clone());
    }
    None
}

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/writeback_provider_export.rs"]
mod tests;
