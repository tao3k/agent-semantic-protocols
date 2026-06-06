//! Provider method execution for the local client backend.

use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;

use agent_semantic_client_core::{
    ByteCount, CacheStatus, ClientMethod, ClientRequest, LanguageId, ProviderRegistrySnapshot,
};
use agent_semantic_client_local_cli::{LocalNativeCliBackend, LocalNativeOutput};

use crate::cache_cli::{
    apply_provider_cache_probe, cache_hit_receipt, provider_cache_probe,
    write_prompt_output_cache_after_provider_success,
    write_search_packet_cache_after_provider_success,
};
use crate::cli_args::ParsedArgs;

pub(crate) fn run_provider_method(
    parsed: ParsedArgs,
    method: ClientMethod,
    language_id: LanguageId,
) -> Result<(), String> {
    let snapshot = ProviderRegistrySnapshot::load(&parsed.project_root)?;
    let forwarded_args = provider_forwarded_args(&method, parsed.forwarded_args);
    let mut request = ClientRequest::new(method, parsed.project_root.clone())
        .with_forwarded_args(forwarded_args)
        .with_language(language_id);
    crate::syntax_query_preflight::validate_syntax_query_request(&request)?;
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
    let request_started_at = std::time::Instant::now();
    let cache_probe = if request.stdin.is_some() {
        None
    } else {
        provider_cache_probe(&parsed.project_root, &snapshot, &request)
    };
    if let Some(cache_probe) = &cache_probe
        && let Some(replay) = &cache_probe.replay
    {
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
    let packet_first_output = if should_try_search_packet_first(&request) {
        run_search_packet_first_miss(
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
        let writeback_snapshot = snapshot.clone();
        let backend = LocalNativeCliBackend::new(snapshot);
        let mut output = backend.execute(&request)?;
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
            apply_provider_cache_probe(&mut output.receipt, writeback_probe);
            output.receipt.cache_status = execution_cache_status;
        }
        output
    };
    crate::syntax_receipt::apply_syntax_query_receipt_metadata(&mut output.receipt, &output.stdout);
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

fn is_stdin_candidate_ingest_request(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Search
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "ingest")
}

fn managed_stdin_bytes() -> Result<Vec<u8>, String> {
    if io::stdin().is_terminal() {
        return Ok(Vec::new());
    }
    let mut stdin = Vec::new();
    io::stdin()
        .read_to_end(&mut stdin)
        .map_err(|error| format!("failed to read provider stdin: {error}"))?;
    Ok(stdin)
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
|note kind=stdin-required message=\"search ingest consumes stdin candidate paths; use search prime --view seeds for project discovery\"\n\
|next prime:\"search prime --view seeds\"(scope=project-discovery),ingest:\"pipe candidate paths into search ingest items tests --view seeds\"(scope=stdin-candidates)\n"
}

fn provider_forwarded_args(method: &ClientMethod, args: Vec<String>) -> Vec<String> {
    if method != &ClientMethod::Query || !args.iter().any(|arg| arg == "--treesitter-query") {
        return args;
    }
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            if arg == "." && !arg_is_option_value(&args, index) {
                None
            } else {
                Some(arg.clone())
            }
        })
        .collect()
}

fn arg_is_option_value(args: &[String], index: usize) -> bool {
    let Some(previous) = index.checked_sub(1).and_then(|previous| args.get(previous)) else {
        return false;
    };
    previous.starts_with("--") && !previous.contains('=')
}

fn should_try_search_packet_first(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Search
        && !is_prime_seed_search(&request.forwarded_args)
        && !request
            .forwarded_args
            .iter()
            .any(|arg| arg == "items" || arg == "ingest" || arg == "--code" || arg == "--json")
        && has_seed_view(&request.forwarded_args)
}

fn is_prime_seed_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "prime") && has_seed_view(args)
}

fn has_seed_view(args: &[String]) -> bool {
    args.windows(2)
        .any(|window| window[0] == "--view" && window[1] == "seeds")
        || args.iter().any(|arg| arg == "--view=seeds")
}

fn run_search_packet_first_miss(
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
    let packet_request = ClientRequest::new(ClientMethod::Search, project_root.to_path_buf())
        .with_forwarded_args(packet_args)
        .with_language(language_id);
    let backend = LocalNativeCliBackend::new(snapshot.clone());
    let mut output = backend.execute(&packet_request)?;
    if output.status_code != 0 {
        return Ok(None);
    }
    let Some(rendered_stdout) = crate::cache_replay::render_search_packet_bytes(&output.stdout)
    else {
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

fn insert_json_flag_before_project_root(args: &mut Vec<String>) {
    let insert_at = if args.last().is_some_and(|arg| arg == ".") {
        args.len().saturating_sub(1)
    } else {
        args.len()
    };
    args.insert(insert_at, "--json".to_string());
}
