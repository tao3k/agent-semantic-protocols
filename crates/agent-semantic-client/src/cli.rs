//! CLI dispatcher for the public `asp` agent semantic client surface.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use agent_semantic_client_core::{
    ByteCount, CacheArtifactId, CacheExportMethod, CacheManifestReport, CacheStatus,
    ClientCacheManifest, ClientCachePath, ClientMethod, ClientReceipt, ClientRequest,
    ElapsedMillis, NativeProvenance, ProviderRegistrySnapshot, ResolvedProvider,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbGenerationHit, ClientDbGenerationLookup, ClientDbReport,
};
use agent_semantic_client_local_cli::LocalNativeCliBackend;

/// Run the `asp` client using process arguments and the current directory.
pub fn run_cli_from_env() -> Result<(), String> {
    let args = env::args().skip(1).collect();
    let cwd = env::current_dir().map_err(|error| format!("failed to read current dir: {error}"))?;
    run_cli_args(args, cwd)
}

/// Run the `asp` client over already-parsed argument strings.
pub fn run_cli_args(args: Vec<String>, cwd: PathBuf) -> Result<(), String> {
    let parsed = ParsedArgs::parse(args, cwd)?;
    match parsed.command.as_deref() {
        None | Some("help" | "--help" | "-h") => {
            print_guide();
            Ok(())
        }
        Some("guide") => run_guide(parsed),
        Some("providers") => run_providers(parsed),
        Some("doctor") => run_doctor(parsed),
        Some("cache") => run_cache(parsed),
        Some("cloud") => run_cloud(parsed),
        Some("search") => run_provider_method(parsed, ClientMethod::Search),
        Some("query") => run_provider_method(parsed, ClientMethod::Query),
        Some("check") => run_provider_method(parsed, ClientMethod::Check),
        Some(command) => Err(format!("unknown asp command `{command}`; try `asp guide`")),
    }
}

fn run_guide(parsed: ParsedArgs) -> Result<(), String> {
    if parsed.language_id.is_none() {
        print_guide();
        return Ok(());
    }
    run_provider_method(parsed, ClientMethod::Guide)
}

fn run_provider_method(parsed: ParsedArgs, method: ClientMethod) -> Result<(), String> {
    let snapshot = ProviderRegistrySnapshot::load(&parsed.project_root)?;
    let request = with_language_if_present(
        ClientRequest::new(method, parsed.project_root.clone())
            .with_forwarded_args(parsed.forwarded_args),
        parsed.language_id,
    );
    let cache_probe = provider_cache_probe(&parsed.project_root, &snapshot, &request);
    if let Some(cache_probe) = &cache_probe {
        if let Some(replay) = &cache_probe.replay {
            io::stdout()
                .write_all(&replay.stdout)
                .map_err(|error| format!("failed to write cached provider stdout: {error}"))?;
            if parsed.receipt_json {
                let receipt = cache_hit_receipt(request.method.clone(), cache_probe, replay);
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            return Ok(());
        }
    }
    let backend = LocalNativeCliBackend::new(snapshot);
    let mut output = backend.execute(&request)?;
    if let Some(cache_probe) = &cache_probe {
        apply_provider_cache_probe(&mut output.receipt, cache_probe);
    }

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
            .map_err(|error| format!("failed to serialize receipt: {error}"))?;
        eprintln!("{receipt}");
    }

    if output.status_code != 0 {
        std::process::exit(output.status_code);
    }
    Ok(())
}

struct ProviderCacheProbe {
    cache_report: CacheManifestReport,
    db_report: ClientDbReport,
    cache_status: CacheStatus,
    provenance: Vec<NativeProvenance>,
    replay: Option<ProviderCacheReplay>,
}

struct ProviderCacheReplay {
    stdout: Vec<u8>,
}

fn provider_cache_probe(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
) -> Option<ProviderCacheProbe> {
    let cache_report = ClientCacheManifest::inspect_project(project_root);
    let cache_root = cache_report.cache_root.as_ref()?;
    let db_path = ClientDb::default_path(cache_root);
    let db_report = ClientDb::inspect(&db_path);
    let selected_provider = selected_provider_for_request(snapshot, request);
    let provenance = selected_provider
        .map(|provider| vec![provider.provenance()])
        .unwrap_or_default();
    let generation_hit = if db_report.status == agent_semantic_client_core::ClientDbStatus::Present
    {
        selected_provider
            .zip(request_export_method(request))
            .and_then(|(provider, export_method)| {
                ClientDb::lookup_generation(&ClientDbGenerationLookup {
                    db_path: db_path.clone(),
                    language_id: provider.language_id.clone(),
                    provider_id: provider.provider_id.clone(),
                    project_root: project_root.to_path_buf(),
                    export_method,
                })
                .ok()
                .flatten()
            })
    } else {
        None
    };
    let replay = generation_hit
        .as_ref()
        .and_then(|hit| load_prompt_output_artifact(cache_root, hit));
    let cache_status = if replay.is_some() {
        CacheStatus::Hit
    } else if generation_hit.is_some() {
        CacheStatus::WarmProvider
    } else {
        CacheStatus::Miss
    };
    Some(ProviderCacheProbe {
        cache_report,
        db_report,
        cache_status,
        provenance,
        replay,
    })
}

fn selected_provider_for_request<'a>(
    snapshot: &'a ProviderRegistrySnapshot,
    request: &ClientRequest,
) -> Option<&'a ResolvedProvider> {
    if let Some(language_id) = &request.language_id {
        return snapshot.provider_for_language(language_id);
    }
    if snapshot.providers.len() == 1 {
        snapshot.providers.first()
    } else {
        None
    }
}

fn request_export_method(request: &ClientRequest) -> Option<CacheExportMethod> {
    let prefix = match &request.method {
        ClientMethod::Search => "search",
        ClientMethod::Query => "query",
        ClientMethod::Check => "check",
        _ => return None,
    };
    let export_method = request
        .forwarded_args
        .first()
        .filter(|arg| !arg.starts_with('-') && arg.as_str() != ".")
        .map_or_else(|| prefix.to_string(), |arg| format!("{prefix}/{arg}"));
    Some(CacheExportMethod::from(export_method))
}

fn apply_provider_cache_probe(receipt: &mut ClientReceipt, probe: &ProviderCacheProbe) {
    receipt.cache_status = probe.cache_status;
    receipt.cache_root = probe
        .cache_report
        .cache_root
        .as_ref()
        .map(|path| ClientCachePath::from_path(path));
    receipt.cache_manifest_path = probe
        .cache_report
        .manifest_path
        .as_ref()
        .map(|path| ClientCachePath::from_path(path));
    receipt.cache_manifest_status = Some(probe.cache_report.status.clone());
    receipt.cache_generation_count = Some(probe.cache_report.generation_count);
    receipt.raw_source_stored = Some(probe.cache_report.raw_source_stored);
    receipt.client_db_path = Some(ClientCachePath::from_path(&probe.db_report.db_path));
    receipt.client_db_status = Some(probe.db_report.status.clone());
    receipt.client_db_generation_count = Some(probe.db_report.generation_count);
    receipt.client_db_raw_source_stored = Some(probe.db_report.raw_source_stored);
}

fn cache_hit_receipt(
    method: ClientMethod,
    probe: &ProviderCacheProbe,
    replay: &ProviderCacheReplay,
) -> ClientReceipt {
    let mut receipt =
        ClientReceipt::cache_report(method, probe.provenance.clone(), &probe.cache_report);
    apply_provider_cache_probe(&mut receipt, probe);
    receipt.cache_status = CacheStatus::Hit;
    receipt.stdout_bytes = ByteCount::from_len(replay.stdout.len());
    receipt.elapsed_ms = ElapsedMillis::new(0);
    receipt
}

fn load_prompt_output_artifact(
    cache_root: &Path,
    generation_hit: &ClientDbGenerationHit,
) -> Option<ProviderCacheReplay> {
    generation_hit
        .artifact_ids
        .iter()
        .find_map(|artifact_id| read_prompt_output_artifact(cache_root, artifact_id))
}

fn read_prompt_output_artifact(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
) -> Option<ProviderCacheReplay> {
    let artifact_path = prompt_output_artifact_path(cache_root, artifact_id)?;
    let metadata = fs::metadata(&artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > 1_048_576 {
        return None;
    }
    let stdout = fs::read(artifact_path).ok()?;
    Some(ProviderCacheReplay { stdout })
}

fn prompt_output_artifact_path(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
) -> Option<PathBuf> {
    let artifact_id = artifact_id.as_str();
    if !artifact_id.starts_with("prompt-output/") || !artifact_id.ends_with(".txt") {
        return None;
    }
    let relative = Path::new(artifact_id);
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(cache_root.parent()?.join("artifacts").join(relative))
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
    println!("|cache status=disabled reason=phase-1-local-native-checkpoint route=local-native");
    println!("|cloud status=disabled reason=local-default privateServer=optional");
    Ok(())
}

fn run_cache(parsed: ParsedArgs) -> Result<(), String> {
    match parsed.forwarded_args.as_slice() {
        [subcommand] if subcommand == "status" => {
            let snapshot = ProviderRegistrySnapshot::load(&parsed.project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(&parsed.project_root);
            let db_report = cache_report
                .cache_root
                .as_ref()
                .map(|cache_root| ClientDb::inspect(ClientDb::default_path(cache_root)));
            let mut receipt = ClientReceipt::cache_status(provenance, &cache_report);
            if let Some(db_report) = &db_report {
                receipt.client_db_path = Some(ClientCachePath::from_path(&db_report.db_path));
                receipt.client_db_status = Some(db_report.status.clone());
                receipt.client_db_generation_count = Some(db_report.generation_count);
                receipt.client_db_raw_source_stored = Some(db_report.raw_source_stored);
            }
            let (activation, provider_count) = match &snapshot {
                Ok(snapshot) => (
                    snapshot.activation_path.display().to_string(),
                    snapshot.providers.len(),
                ),
                Err(error) => {
                    if !parsed.receipt_json {
                        eprintln!("[asp-cache] activation unavailable: {error}");
                    }
                    ("missing".to_string(), 0)
                }
            };
            println!(
                "[asp-cache] status=disabled route=local-cache activation={} providers={} cacheRoot={} manifest={} generations={} rawSourceStored={}",
                activation,
                provider_count,
                display_optional_path(cache_report.cache_root.as_deref()),
                cache_report.status.as_str(),
                cache_report.generation_count,
                cache_report.raw_source_stored
            );
            println!(
                "|cache manifestPath={} cacheManifestStatus={}",
                display_optional_path(cache_report.manifest_path.as_deref()),
                cache_report.status.as_str()
            );
            print_db_status(db_report.as_ref());
            print_cache_reason(&cache_report);
            println!("|reason phase=phase-1-client-db-sql arrow=false providerCommands=0");
            if snapshot.is_err() {
                println!("|cmd install=asp hook install --client codex .");
                println!("|cmd guide=asp guide");
            }
            if parsed.receipt_json {
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand] if subcommand == "import" => {
            let snapshot = ProviderRegistrySnapshot::load(&parsed.project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(&parsed.project_root);
            let manifest = ClientCacheManifest::load_from_path(
                cache_report
                    .manifest_path
                    .as_ref()
                    .ok_or_else(|| "cache manifest path unavailable".to_string())?,
            )?;
            let cache_root = cache_report
                .cache_root
                .as_ref()
                .ok_or_else(|| "cache root unavailable".to_string())?;
            let db_path = ClientDb::default_path(cache_root);
            let mut db = ClientDb::open_or_create(db_path.clone())?;
            db.import_manifest(&manifest)?;
            let db_report = ClientDb::inspect(db_path);
            let mut receipt =
                ClientReceipt::cache_report(ClientMethod::CacheImport, provenance, &cache_report);
            receipt.client_db_path = Some(ClientCachePath::from_path(&db_report.db_path));
            receipt.client_db_status = Some(db_report.status.clone());
            receipt.client_db_generation_count = Some(db_report.generation_count);
            receipt.client_db_raw_source_stored = Some(db_report.raw_source_stored);
            println!(
                "[asp-cache] status=imported route=local-cache cacheRoot={} manifest={} generations={} rawSourceStored={}",
                display_optional_path(cache_report.cache_root.as_deref()),
                cache_report.status.as_str(),
                cache_report.generation_count,
                cache_report.raw_source_stored
            );
            println!(
                "|cache manifestPath={} cacheManifestStatus={}",
                display_optional_path(cache_report.manifest_path.as_deref()),
                cache_report.status.as_str()
            );
            print_db_status(Some(&db_report));
            println!(
                "|reason phase=phase-1-client-db-sql action=import arrow=false providerCommands=0"
            );
            if parsed.receipt_json {
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            Ok(())
        }
        _ => Err("usage: asp cache <status|import> [--root <path>]".to_string()),
    }
}

fn print_db_status(db_report: Option<&agent_semantic_client_db::ClientDbReport>) {
    if let Some(db_report) = db_report {
        println!(
            "|db path={} status={} generations={} rawSourceStored={}",
            db_report.db_path.display(),
            db_report.status.as_str(),
            db_report.generation_count,
            db_report.raw_source_stored
        );
        if let Some(reason) = &db_report.reason {
            println!(
                "|reason clientDb={} detail={}",
                db_report.status.as_str(),
                compact_detail(reason)
            );
        }
    } else {
        println!("|db path=unavailable status=unavailable generations=0 rawSourceStored=false");
    }
}

fn print_cache_reason(cache_report: &CacheManifestReport) {
    if let Some(reason) = &cache_report.reason {
        println!(
            "|reason cacheManifest={} detail={}",
            cache_report.status.as_str(),
            compact_detail(reason)
        );
    }
}

fn display_optional_path(path: Option<&std::path::Path>) -> String {
    path.map_or_else(
        || "unavailable".to_string(),
        |path| path.display().to_string(),
    )
}

fn compact_detail(detail: &str) -> String {
    detail.split_whitespace().collect::<Vec<_>>().join("_")
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
    println!("|cmd search=asp search --language <rust|typescript|python> <provider-search-args>");
    println!("|cmd query=asp query --language <rust|typescript|python> <provider-query-args>");
    println!("|cmd check=asp check --language <rust|typescript|python> <provider-check-args>");
    println!("|cmd cache=asp cache status");
    println!("|cmd cache-import=asp cache import");
    println!("|cmd cloud=asp cloud status");
    println!("|rule route=local-native cache=disabled cloud=optional nativeProviderFacts=required");
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedArgs {
    command: Option<String>,
    language_id: Option<String>,
    project_root: PathBuf,
    forwarded_args: Vec<String>,
    receipt_json: bool,
}

impl ParsedArgs {
    fn parse(args: Vec<String>, cwd: PathBuf) -> Result<Self, String> {
        let mut command = None;
        let mut language_id = None;
        let mut project_root = cwd;
        let mut forwarded_args = Vec::new();
        let mut receipt_json = false;
        let mut iter = args.into_iter();

        if let Some(first) = iter.next() {
            command = Some(first);
        }

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--language" => {
                    language_id = Some(
                        iter.next()
                            .ok_or_else(|| "--language requires a value".to_string())?,
                    );
                }
                "--root" => {
                    project_root = PathBuf::from(
                        iter.next()
                            .ok_or_else(|| "--root requires a value".to_string())?,
                    );
                }
                "--receipt-json" => {
                    receipt_json = true;
                }
                _ => forwarded_args.push(arg),
            }
        }

        Ok(Self {
            command,
            language_id,
            project_root,
            forwarded_args,
            receipt_json,
        })
    }
}

fn with_language_if_present(request: ClientRequest, language_id: Option<String>) -> ClientRequest {
    if let Some(language_id) = language_id {
        request.with_language(language_id)
    } else {
        request
    }
}
