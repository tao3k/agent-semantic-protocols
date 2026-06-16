//! Provider method execution for the local client backend.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    ByteCount, CacheManifestStatus, CacheStatus, ClientCacheManifest, ClientMethod, ClientRequest,
    LanguageId, ProviderRegistrySnapshot,
};
use agent_semantic_client_local_cli::{LocalNativeCliBackend, LocalNativeOutput};
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode, run_provider_process,
};
use bytes::Bytes;
use sha2::{Digest, Sha256};

use crate::cache_cli::{
    apply_provider_cache_probe, cache_hit_receipt, provider_cache_probe,
    write_prompt_output_cache_after_provider_success,
    write_query_packet_cache_after_provider_success,
    write_search_packet_cache_after_provider_success,
};
use crate::cli_args::ParsedArgs;

const ASP_DEBUG_CLIENT_STAGE_ENV: &str = "ASP_DEBUG_CLIENT_STAGE";

fn debug_client_stage(stage: &str) {
    if env::var_os(ASP_DEBUG_CLIENT_STAGE_ENV).is_some() {
        eprintln!("[asp-client-stage] {stage}");
    }
}

pub(crate) fn run_provider_method(
    parsed: ParsedArgs,
    method: ClientMethod,
    language_id: LanguageId,
) -> Result<(), String> {
    debug_client_stage("provider-method:load-registry");
    let snapshot = crate::activation_cache::load_provider_registry_snapshot(
        &parsed.activation_root,
        &parsed.project_root,
        !parsed.receipt_json,
    )?;
    debug_client_stage("provider-method:forward-args");
    let check_failure_frontier_view =
        method == ClientMethod::Check && has_seed_view(&parsed.forwarded_args);
    let forwarded_args = provider_forwarded_args(&method, parsed.forwarded_args);
    let request_language_id = language_id.clone();
    debug_client_stage("provider-method:new-request");
    let mut request = ClientRequest::new(method, parsed.project_root.clone())
        .with_forwarded_args(forwarded_args)
        .with_language(language_id);
    debug_client_stage("provider-method:syntax-preflight");
    crate::syntax_query_preflight::validate_syntax_query_request(&request)?;
    debug_client_stage("provider-method:stdin-check");
    if is_stdin_candidate_ingest_request(&request) {
        let stdin = managed_stdin_bytes()?;
        if stdin.is_empty() && wants_agent_compact_output(&request.forwarded_args) {
            io::stdout()
                .write_all(empty_ingest_diagnostic().as_bytes())
                .map_err(|error| format!("failed to write empty ingest diagnostic: {error}"))?;
            return Ok(());
        }
        request = request.with_stdin(stdin);
    }
    debug_client_stage("provider-method:cache-probe");
    let request_started_at = std::time::Instant::now();
    let cache_probe = if request.stdin.is_some() {
        None
    } else {
        provider_cache_probe(&parsed.project_root, &snapshot, &request)
    };
    debug_client_stage("provider-method:cache-replay");
    if let Some(cache_probe) = &cache_probe
        && let Some(replay) = &cache_probe.replay
    {
        crate::compact_mode::validate_compact_provider_stdout(&request, &replay.stdout)?;
        io::stdout()
            .write_all(&replay.stdout)
            .map_err(|error| format!("failed to write cache replay stdout: {error}"))?;
        if parsed.receipt_json {
            let mut receipt = cache_hit_receipt(
                request.method.clone(),
                cache_probe,
                replay,
                agent_semantic_client_core::ElapsedMillis::new(
                    request_started_at
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                ),
            );
            crate::syntax_receipt::apply_syntax_query_receipt_metadata(
                &mut receipt,
                &replay.stdout,
            );
            let receipt = serde_json::to_string(&receipt)
                .map_err(|error| format!("failed to serialize receipt JSON: {error}"))?;
            eprintln!("{receipt}");
        }
        return Ok(());
    }
    let execution_cache_status = cache_probe
        .as_ref()
        .map_or(CacheStatus::Miss, |probe| probe.cache_status);
    debug_client_stage("provider-method:manifest-policy");
    let cache_manifest_allows_packet_first =
        cache_manifest_allows_packet_first(&parsed.project_root);
    debug_client_stage("provider-method:packet-first");
    let packet_first_output =
        if cache_manifest_allows_packet_first && should_try_search_packet_first(&request) {
            run_search_packet_first_miss(
                &parsed.project_root,
                &snapshot,
                &request,
                execution_cache_status,
                parsed.frontier_receipt_out.as_deref(),
            )?
        } else if cache_manifest_allows_packet_first && should_try_query_packet_first(&request) {
            run_query_packet_first_miss(
                &parsed.project_root,
                &snapshot,
                &request,
                execution_cache_status,
            )?
        } else {
            None
        };
    let mut output = if let Some(output) = packet_first_output {
        output
    } else {
        debug_client_stage("provider-method:clone-snapshot");
        let writeback_snapshot = snapshot.clone();
        debug_client_stage("provider-method:new-backend");
        let backend = LocalNativeCliBackend::new(snapshot);
        debug_client_stage("provider-method:execute");
        let mut output = backend.execute(&request)?;
        debug_client_stage("provider-method:execute-done");
        if output.status_code == 0 {
            crate::compact_mode::validate_compact_provider_stdout(&request, &output.stdout)?;
        }
        let writeback_probe = if output.status_code == 0 && request.stdin.is_none() {
            write_prompt_output_cache_after_provider_success(
                &parsed.project_root,
                &writeback_snapshot,
                &request,
                &output.stdout,
                &output.receipt.provider_commands,
            )
        } else {
            None
        };
        if let Some(cache_probe) = &cache_probe {
            apply_provider_cache_probe(&mut output.receipt, cache_probe);
        }
        let execution_cache_status = output.receipt.cache_status;
        if let Some(writeback_probe) = &writeback_probe {
            if let Some(cache_probe) = &writeback_probe.cache_probe {
                apply_provider_cache_probe(&mut output.receipt, cache_probe);
                output.receipt.cache_status = execution_cache_status;
            }
            if !writeback_probe.provider_commands.is_empty() {
                let command_count = writeback_probe
                    .provider_commands
                    .len()
                    .min(u32::MAX as usize) as u32;
                output.receipt.cache_writeback_provider_command_count = Some(command_count);
                output.receipt.cache_writeback_provider_processes_spawned = Some(command_count);
                output.receipt.cache_writeback_provider_elapsed_ms =
                    Some(writeback_probe.provider_elapsed_ms);
                output.receipt.cache_writeback_provider_commands =
                    Some(writeback_probe.provider_commands.clone());
            }
        }
        output
    };
    if output.status_code == 0 {
        crate::compact_mode::validate_compact_provider_stdout(&request, &output.stdout)?;
    }
    crate::syntax_receipt::apply_syntax_query_receipt_metadata(&mut output.receipt, &output.stdout);
    if request.method == ClientMethod::Check {
        persist_last_check_output(
            &parsed.project_root,
            output.status_code,
            &output.stdout,
            &output.stderr,
        )?;
        if output.status_code != 0 && check_failure_frontier_view {
            let frontier =
                render_last_check_failure_frontier(&parsed.project_root, &request_language_id)?;
            io::stdout()
                .write_all(frontier.as_ref())
                .map_err(|error| format!("failed to write failure frontier stdout: {error}"))?;
            if parsed.receipt_json {
                let receipt = serde_json::to_string(&output.receipt)
                    .map_err(|error| format!("failed to serialize receipt JSON: {error}"))?;
                eprintln!("{receipt}");
            }
            return Ok(());
        }
    }
    debug_client_stage("provider-method:backend");
    if !parsed.receipt_json {
        io::stderr()
            .write_all(&output.stderr)
            .map_err(|error| format!("failed to write provider stderr: {error}"))?;
    }
    io::stdout()
        .write_all(&output.stdout)
        .map_err(|error| format!("failed to write provider stdout: {error}"))?;
    if parsed.receipt_json {
        let receipt = serde_json::to_string(&output.receipt)
            .map_err(|error| format!("failed to serialize receipt JSON: {error}"))?;
        eprintln!("{receipt}");
    }
    if output.status_code != 0 {
        std::process::exit(output.status_code);
    }
    Ok(())
}

pub(crate) fn persist_last_check_output(
    project_root: &Path,
    status_code: i32,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), String> {
    let path = last_check_output_path(project_root);
    if status_code == 0 {
        let _ = fs::remove_file(path);
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut transcript = Vec::new();
    transcript.extend_from_slice(stdout);
    if !stdout.is_empty() && !stdout.ends_with(b"\n") {
        transcript.push(b'\n');
    }
    transcript.extend_from_slice(stderr);
    fs::write(&path, transcript)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(crate) fn last_check_output_path(project_root: &Path) -> PathBuf {
    let cache_home = env::var_os("PRJ_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.join(".cache"));
    cache_home
        .join("agent-semantic-protocol")
        .join("last-check-output.txt")
}

fn is_stdin_candidate_ingest_request(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Search
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "ingest")
}

fn managed_stdin_bytes() -> Result<Bytes, String> {
    if io::stdin().is_terminal() {
        return Ok(Bytes::new());
    }
    let mut stdin = Vec::new();
    io::stdin()
        .read_to_end(&mut stdin)
        .map_err(|error| format!("failed to read provider stdin: {error}"))?;
    Ok(Bytes::from(stdin))
}

fn wants_agent_compact_output(args: &[String]) -> bool {
    !args.iter().any(|arg| arg == "--json" || arg == "--code")
}

fn empty_ingest_diagnostic() -> &'static str {
    "[search-ingest] root=. alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search}\n\
G>{}\n\
rank= frontier=\n\
|note kind=stdin-required message=\"search ingest consumes stdin candidate paths; use search prime --workspace . --view seeds for project discovery\"\n\
|next prime:\"search prime --workspace . --view seeds\"(scope=project-discovery),ingest:\"pipe candidate paths into search ingest items tests --view seeds\"(scope=stdin-candidates)\n"
}

fn provider_forwarded_args(method: &ClientMethod, args: Vec<String>) -> Vec<String> {
    if method == &ClientMethod::Check {
        return normalize_check_forwarded_args(args);
    }
    args
}

fn normalize_check_forwarded_args(args: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "changed" => {
                normalized.push("--changed".to_string());
                index += 1;
            }
            "--view" if args.get(index + 1).is_some_and(|value| value == "seeds") => {
                index += 2;
            }
            "--view=seeds" => {
                index += 1;
            }
            _ => {
                normalized.push(args[index].clone());
                index += 1;
            }
        }
    }
    normalized
}

fn render_last_check_failure_frontier(
    project_root: &Path,
    language_id: &LanguageId,
) -> Result<Vec<u8>, String> {
    let program = env::var_os("SEMANTIC_AGENT_PROTOCOL_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("asp"))
        .to_string_lossy()
        .into_owned();
    let output = run_provider_process(ProviderProcessSpec {
        program,
        args: vec![
            language_id.to_string(),
            "search".to_string(),
            "failure".to_string(),
            "--from-last-check".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ],
        cwd: project_root.to_path_buf(),
        env: BTreeMap::new(),
        stdin: StdinMode::Closed,
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    })
    .map_err(|error| format!("failed to render check failure frontier: {error}"))?;
    if !output.stderr.is_empty() {
        io::stderr()
            .write_all(output.stderr.as_ref())
            .map_err(|error| format!("failed to write failure frontier stderr: {error}"))?;
    }
    if !output.status.success() {
        return Err(format!(
            "search failure frontier exited with status {}",
            output.status.code().unwrap_or(1)
        ));
    }
    Ok(output.stdout.to_vec())
}

pub(crate) fn should_try_search_packet_first(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Search
        && !is_workspace_seed_search(&request.forwarded_args)
        && !is_compare_search(&request.forwarded_args)
        && !request
            .forwarded_args
            .iter()
            .any(|arg| arg == "items" || arg == "ingest" || arg == "--code" || arg == "--json")
        && (is_prime_seed_search(&request.forwarded_args)
            || is_search_packet_seed_search(&request.forwarded_args)
            || is_dependency_search(&request.forwarded_args))
}

pub(crate) fn should_try_query_packet_first(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Query
        && request
            .forwarded_args
            .iter()
            .any(|arg| arg == "--names-only")
        && !request.forwarded_args.iter().any(|arg| {
            arg == "--json"
                || arg == "--code"
                || arg == "--treesitter-query"
                || arg == "--catalog"
                || arg == "--from-hook"
        })
        && request.forwarded_args.iter().any(|arg| {
            arg == "--term"
                || arg == "--query"
                || arg.starts_with("--term=")
                || arg.starts_with("--query=")
        })
        && request
            .forwarded_args
            .iter()
            .any(|arg| !arg.starts_with('-') && arg != ".")
}

fn cache_manifest_allows_packet_first(project_root: &Path) -> bool {
    matches!(
        ClientCacheManifest::inspect_project(project_root).status,
        CacheManifestStatus::Missing | CacheManifestStatus::Present
    )
}

fn is_prime_seed_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "prime") && has_seed_view(args)
}

fn is_workspace_seed_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "workspace") && has_seed_view(args)
}

fn is_compare_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "compare")
}

fn is_search_packet_seed_search(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| arg == "fzf" || arg == "pipe")
        && has_seed_view(args)
}

fn has_seed_view(args: &[String]) -> bool {
    args.windows(2)
        .any(|window| window[0] == "--view" && window[1] == "seeds")
        || args.iter().any(|arg| arg == "--view=seeds")
}

fn is_dependency_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "deps")
}

fn run_search_packet_first_miss(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    execution_cache_status: CacheStatus,
    frontier_receipt_out: Option<&Path>,
) -> Result<Option<LocalNativeOutput>, String> {
    let Some(language_id) = request.language_id.clone() else {
        return Ok(None);
    };
    let mut packet_args = request.forwarded_args.clone();
    insert_json_flag_before_project_root(&mut packet_args);
    let packet_request = ClientRequest::new(ClientMethod::Search, project_root.to_path_buf())
        .with_forwarded_args(packet_args)
        .with_language(language_id);
    let backend = LocalNativeCliBackend::new(snapshot.clone());
    let mut output = backend.execute(&packet_request)?;
    if output.status_code != 0 {
        return Ok(None);
    }
    let receipt_request = frontier_receipt_out
        .map(|path| search_frontier_receipt_request(path, request, &output.stdout));
    let rendered_stdout = if let Some(receipt_request) = receipt_request.as_ref() {
        crate::cache_replay::render_search_packet_bytes_with_receipt(
            output.stdout.clone(),
            receipt_request,
        )?
    } else {
        crate::cache_replay::render_search_packet_bytes(output.stdout.clone())
    };
    let Some(rendered_stdout) = rendered_stdout else {
        return Ok(None);
    };
    let Some(writeback_probe) = write_search_packet_cache_after_provider_success(
        project_root,
        snapshot,
        request,
        &output.stdout,
        &rendered_stdout,
    ) else {
        return Ok(None);
    };
    apply_provider_cache_probe(&mut output.receipt, &writeback_probe);
    output.receipt.cache_status = execution_cache_status;
    output.receipt.packet_bytes = Some(ByteCount::from_len(output.stdout.len()));
    output.receipt.stdout_bytes = ByteCount::from_len(rendered_stdout.len());
    output.stdout = rendered_stdout;
    Ok(Some(output))
}

fn run_query_packet_first_miss(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    execution_cache_status: CacheStatus,
) -> Result<Option<LocalNativeOutput>, String> {
    let Some(language_id) = request.language_id.clone() else {
        return Ok(None);
    };
    let mut packet_args = request.forwarded_args.clone();
    insert_json_flag_before_project_root(&mut packet_args);
    let packet_request = ClientRequest::new(ClientMethod::Query, project_root.to_path_buf())
        .with_forwarded_args(packet_args)
        .with_language(language_id);
    let backend = LocalNativeCliBackend::new(snapshot.clone());
    let mut output = backend.execute(&packet_request)?;
    if output.status_code != 0 {
        return Ok(None);
    }
    let Some(rendered_stdout) =
        crate::cache_replay::render_query_packet_bytes(output.stdout.clone())
    else {
        return Ok(None);
    };
    let Some(writeback_probe) = write_query_packet_cache_after_provider_success(
        project_root,
        snapshot,
        request,
        &output.stdout,
    ) else {
        return Ok(None);
    };
    apply_provider_cache_probe(&mut output.receipt, &writeback_probe);
    output.receipt.cache_status = execution_cache_status;
    output.receipt.packet_bytes = Some(ByteCount::from_len(output.stdout.len()));
    output.receipt.stdout_bytes = ByteCount::from_len(rendered_stdout.len());
    output.stdout = rendered_stdout;
    Ok(Some(output))
}

fn search_frontier_receipt_request(
    out_path: &Path,
    request: &ClientRequest,
    packet_bytes: &[u8],
) -> crate::cache_replay::SearchFrontierReceiptRequest {
    let packet_hash = short_sha256(packet_bytes);
    let language_id = request
        .language_id
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    crate::cache_replay::SearchFrontierReceiptRequest {
        out_path: out_path.to_path_buf(),
        receipt_id: format!("asp.search-frontier.{language_id}.{packet_hash}"),
        task_fingerprint: format!("task:asp-search-frontier:{language_id}:{packet_hash}"),
        command_fingerprint: format!("command:asp-search:{language_id}:{packet_hash}"),
    }
}

fn short_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}").chars().take(16).collect()
}

fn insert_json_flag_before_project_root(args: &mut Vec<String>) {
    let insert_at = if args.last().is_some_and(|arg| arg == ".") {
        args.len().saturating_sub(1)
    } else {
        args.len()
    };
    args.insert(insert_at, "--json".to_string());
}
