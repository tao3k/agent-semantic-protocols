//! Provider method execution for the local client backend.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{ClientMethod, ClientRequest, LanguageId};
use agent_semantic_client_local_cli::LocalNativeCliBackend;
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, ProviderProcessSupervisor, StdinMode,
};
use bytes::Bytes;

use crate::cli_args::ParsedArgs;

const ASP_DEBUG_CLIENT_STAGE_ENV: &str = "ASP_DEBUG_CLIENT_STAGE";

fn debug_client_stage(stage: &str) {
    if env::var_os(ASP_DEBUG_CLIENT_STAGE_ENV).is_some() {
        eprintln!("[asp-client-stage] {stage}");
    }
}

pub(crate) async fn run_provider_method(
    supervisor: ProviderProcessSupervisor,
    parsed: ParsedArgs,
    method: ClientMethod,
    language_id: LanguageId,
) -> Result<(), String> {
    let provider_method_name = serde_json::to_string(&method)
        .map_err(|error| format!("serialize provider method: {error}"))?
        .trim_matches('"')
        .to_owned();
    match crate::provider_runtime_storage::ProviderRuntimeStorageBinding::from_current_runtime(
        &parsed.project_root,
    ) {
        Ok(Some(storage)) => {
            if let Err(error) =
                storage.record_invocation_start(provider_method_name.as_str(), language_id.clone())
            {
                debug_client_stage(&format!(
                    "provider-method:runtime-storage-record-unavailable:{error}"
                ));
            }
        }
        Ok(None) => {}
        Err(error) => debug_client_stage(&format!(
            "provider-method:runtime-storage-open-unavailable:{error}"
        )),
    }
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
    snapshot
        .provider_for_language(&request_language_id)
        .ok_or_else(|| format!("provider is missing for language {}", request_language_id))?;
    let mut output = {
        debug_client_stage("provider-method:new-backend");
        let backend = LocalNativeCliBackend::new(snapshot, supervisor.clone());
        debug_client_stage("provider-method:execute");
        let output = backend.execute(&request).await?;
        debug_client_stage("provider-method:execute-done");
        if output.status_code == 0 {
            crate::compact_mode::validate_compact_provider_stdout(&request, &output.stdout)?;
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
            let frontier = render_last_check_failure_frontier(
                &supervisor,
                &parsed.project_root,
                &request_language_id,
            )
            .await?;
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
    !args
        .iter()
        .any(|arg| arg == "--json" || arg == "--projection" || arg.starts_with("--projection="))
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

async fn render_last_check_failure_frontier(
    supervisor: &ProviderProcessSupervisor,
    project_root: &Path,
    language_id: &LanguageId,
) -> Result<Vec<u8>, String> {
    let program = env::var_os("SEMANTIC_AGENT_PROTOCOL_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("asp"))
        .to_string_lossy()
        .into_owned();
    let output = supervisor
        .run(ProviderProcessSpec {
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
        .await
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

fn has_seed_view(args: &[String]) -> bool {
    args.windows(2)
        .any(|window| window[0] == "--view" && window[1] == "seeds")
        || args.iter().any(|arg| arg == "--view=seeds")
}
