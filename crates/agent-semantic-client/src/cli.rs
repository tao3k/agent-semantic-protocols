//! CLI dispatcher for the public `asp` agent semantic client surface.

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{ClientMethod, ClientRequest, ProviderRegistrySnapshot};
use agent_semantic_client_local_cli::LocalNativeCliBackend;

use crate::cache_cli::{
    apply_provider_cache_probe, cache_hit_receipt, provider_cache_probe,
    write_prompt_output_cache_after_provider_success,
};

/// Runs the agent semantic client CLI from process arguments.
pub fn run_cli_from_env() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if matches!(
        args.first().map(String::as_str),
        Some("search" | "query" | "check")
    ) {
        return Err(
            "top-level asp search/query/check has been removed; use asp <rust|typescript|python> <search|query|check> ..."
                .to_string(),
        );
    }
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    run_cli_args(None, args, cwd)
}

/// Runs the agent semantic client CLI with an optional facade language.
pub fn run_cli_args(
    language_id: Option<agent_semantic_client_core::LanguageId>,
    args: Vec<String>,
    cwd: PathBuf,
) -> Result<(), String> {
    let parsed = ParsedArgs::parse(args, cwd, language_id.is_some())?;
    match parsed.command.as_deref() {
        None | Some("help" | "--help" | "-h") => {
            print_guide();
            Ok(())
        }
        Some("guide") => run_guide(parsed, language_id),
        Some("providers") => run_providers(parsed),
        Some("doctor") => run_doctor(parsed),
        Some("cache") => crate::cache_cli::run_cache(
            &parsed.project_root,
            &parsed.forwarded_args,
            parsed.receipt_json,
        ),
        Some("cloud") => run_cloud(parsed),
        Some("search") => run_provider_method(
            parsed,
            ClientMethod::Search,
            language_id.ok_or_else(|| provider_language_required("search"))?,
        ),
        Some("query") => run_provider_method(
            parsed,
            ClientMethod::Query,
            language_id.ok_or_else(|| provider_language_required("query"))?,
        ),
        Some("check") => run_provider_method(
            parsed,
            ClientMethod::Check,
            language_id.ok_or_else(|| provider_language_required("check"))?,
        ),
        Some(command) => Err(format!("unknown client command: {command}")),
    }
}
fn run_guide(
    parsed: ParsedArgs,
    language_id: Option<agent_semantic_client_core::LanguageId>,
) -> Result<(), String> {
    let Some(language_id) = language_id else {
        print_guide();
        return Ok(());
    };
    run_provider_method(parsed, ClientMethod::Guide, language_id)
}

fn provider_language_required(command: &str) -> String {
    format!(
        "asp {command} requires a language facade; use asp <rust|typescript|python> {command} ..."
    )
}

fn run_provider_method(
    parsed: ParsedArgs,
    method: ClientMethod,
    language_id: agent_semantic_client_core::LanguageId,
) -> Result<(), String> {
    let snapshot = ProviderRegistrySnapshot::load(&parsed.project_root)?;
    let request = ClientRequest::new(method, parsed.project_root.clone())
        .with_forwarded_args(parsed.forwarded_args)
        .with_language(language_id);
    let request_started_at = std::time::Instant::now();
    let cache_probe = provider_cache_probe(&parsed.project_root, &snapshot, &request);
    if let Some(cache_probe) = &cache_probe {
        if let Some(replay) = &cache_probe.replay {
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
    }
    let writeback_snapshot = snapshot.clone();
    let backend = LocalNativeCliBackend::new(snapshot);
    let mut output = backend.execute(&request)?;
    let writeback_probe = if output.status_code == 0 {
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

fn run_providers(parsed: ParsedArgs) -> Result<(), String> {
    match ProviderRegistrySnapshot::load(&parsed.project_root) {
        Ok(snapshot) => {
            println!(
                "[asp-providers] activation={} providers={}",
                snapshot.activation_path.display(),
                snapshot.providers.len()
            );
            for provider in snapshot.providers {
                println!(
                    "|provider language={} provider={} binary={} packageRoots={}",
                    provider.language_id,
                    provider.provider_id,
                    provider.binary,
                    provider.package_roots.join(",")
                );
            }
        }
        Err(error) => {
            println!("[asp-providers] activation=missing providers=0");
            println!("|reason provider-activation-unavailable");
            println!("|cmd install=asp hook install --client codex .");
            println!("|cmd guide=asp guide");
            eprintln!("[asp-providers] activation unavailable: {error}");
        }
    }
    Ok(())
}

fn run_doctor(parsed: ParsedArgs) -> Result<(), String> {
    match ProviderRegistrySnapshot::load(&parsed.project_root) {
        Ok(snapshot) => println!(
            "[asp-doctor] status=ok backend=local activation={} providers={} server=not-required",
            snapshot.activation_path.display(),
            snapshot.providers.len()
        ),
        Err(error) => {
            println!(
                "[asp-doctor] status=degraded backend=local activation=missing providers=0 server=not-required"
            );
            println!("|reason provider-activation-unavailable");
            println!("|cmd install=asp hook install --client codex .");
            println!("|cmd guide=asp guide");
            eprintln!("[asp-doctor] activation unavailable: {error}");
        }
    }
    println!(
        "|cache status=inspectable route=local-cache import=manual invalidate=manual replay=artifact-only"
    );
    println!("|cloud status=disabled reason=local-default privateServer=optional");
    Ok(())
}

fn run_cloud(parsed: ParsedArgs) -> Result<(), String> {
    match parsed.forwarded_args.as_slice() {
        [subcommand] if subcommand == "status" => {
            println!(
                "[asp-cloud] status=disabled backend=local privateServer=optional uploadPolicy=none"
            );
            Ok(())
        }
        _ => Err("usage: asp cloud status".to_string()),
    }
}

fn print_guide() {
    println!("[asp-guide] backend=local prompt=compact json=artifact-only");
    println!("|cmd doctor=asp doctor");
    println!("|cmd providers=asp providers");
    println!("|cmd search=asp <rust|typescript|python> search <provider-search-args>");
    println!("|cmd query=asp <rust|typescript|python> query <provider-query-args>");
    println!("|cmd check=asp <rust|typescript|python> check <provider-check-args>");
    println!("|cmd cache=asp cache status");
    println!("|cmd cache-import=asp cache import");
    println!("|cmd cache-invalidate=asp cache invalidate");
    println!("|cmd cloud=asp cloud status");
    println!(
        "|rule route=local-native cache=probe-first cloud=optional nativeProviderFacts=required"
    );
}

struct ParsedArgs {
    command: Option<String>,
    project_root: PathBuf,
    forwarded_args: Vec<String>,
    receipt_json: bool,
}

impl ParsedArgs {
    fn parse(
        args: Vec<String>,
        cwd: PathBuf,
        allow_provider_language_args: bool,
    ) -> Result<Self, String> {
        let mut command = None;
        let mut project_root = cwd;
        let mut explicit_project_root = false;
        let mut forwarded_args = Vec::new();
        let mut receipt_json = false;
        let mut iter = args.into_iter();
        if let Some(first) = iter.next() {
            command = Some(first);
        }
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--language" if !allow_provider_language_args => {
                    return Err("--language has been removed; use asp <rust|typescript|python> <search|query|check> ...".to_string());
                }
                "--root" => {
                    project_root = PathBuf::from(
                        iter.next()
                            .ok_or_else(|| "--root requires a value".to_string())?,
                    );
                    explicit_project_root = true;
                }
                "--receipt-json" => {
                    receipt_json = true;
                }
                _ => forwarded_args.push(arg),
            }
        }
        if !explicit_project_root && should_infer_positional_project_root(command.as_deref()) {
            if let Some(root) = positional_project_root(&forwarded_args, &project_root) {
                project_root = root;
                if let Some(last) = forwarded_args.last_mut() {
                    *last = ".".to_string();
                }
            }
        }
        Ok(Self {
            command,
            project_root,
            forwarded_args,
            receipt_json,
        })
    }
}

fn should_infer_positional_project_root(command: Option<&str>) -> bool {
    matches!(command, Some("search" | "query" | "check"))
}

fn positional_project_root(forwarded_args: &[String], cwd: &Path) -> Option<PathBuf> {
    let value = forwarded_args.last()?;
    if value.starts_with('-') {
        return None;
    }
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    if value == "."
        || absolute
            .join(".cache/agent-semantic-protocol/hooks/activation.json")
            .is_file()
        || absolute
            .join(".cache/agent-semantic-protocol/client/cache-manifest.json")
            .is_file()
        || absolute.join("Cargo.toml").is_file()
        || absolute.join("package.json").is_file()
        || absolute.join("pyproject.toml").is_file()
        || absolute.join("Project.toml").is_file()
        || absolute.join("JuliaProject.toml").is_file()
    {
        Some(absolute)
    } else {
        None
    }
}
