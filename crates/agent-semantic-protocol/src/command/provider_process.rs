use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessOutput, ProviderProcessSpec, StdinMode,
    run_provider_process as run_transport_process,
};
use agent_semantic_runtime::{project_state_paths, runtime_bin_dir_for_cache_home};
use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::install_provider_target::{home_dir, resolve_provider_binary_invocation};
use super::search_config::AspConfig;

const ASP_PROVIDER_TIMEOUT_MS_ENV: &str = "ASP_PROVIDER_TIMEOUT_MS";

pub(super) fn run_provider_command(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
) -> Result<(), String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    let output = run_provider_process(
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
    )?;
    write_facade_stream(language_id, provider, output.stderr.as_ref(), io::stderr())?;
    write_facade_stream(language_id, provider, output.stdout.as_ref(), io::stdout())?;
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }
    Ok(())
}

pub(super) fn run_owner_items_provider_command(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
    owner_path: &str,
) -> Result<(), String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    let output = run_provider_process(
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
    )?;
    write_facade_stream(language_id, provider, output.stderr.as_ref(), io::stderr())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "provider-owned owner-items failed for {owner_path}: {}",
            stderr.trim()
        ));
    }
    if output.stdout.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(format!(
            "provider-owned owner-items produced empty output for {owner_path}"
        ));
    }
    write_facade_stream(language_id, provider, output.stdout.as_ref(), io::stdout())
}

pub(super) fn run_provider_command_with_stdin(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
    stdin: Vec<u8>,
) -> Result<ProviderProcessOutput, String> {
    let limits = default_provider_process_limits()?;
    run_provider_command_with_stdin_limits(
        language_id,
        provider,
        invocation,
        project_root,
        cache_home,
        stdin,
        limits,
    )
}

pub(super) fn run_provider_command_with_stdin_limits(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
    stdin: Vec<u8>,
    limits: ProviderProcessLimits,
) -> Result<ProviderProcessOutput, String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    run_provider_process_with_stdin(ProviderProcessRun {
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
        limits,
        stdin: StdinMode::bytes(stdin),
    })
}

pub(super) fn run_guide_command(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
) -> Result<(), String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    let output = run_provider_process(
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
    )?;
    io::stderr()
        .write_all(&output.stderr)
        .map_err(|error| format!("failed to write provider stderr: {error}"))?;
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let stdout = std::str::from_utf8(output.stdout.as_ref())
        .map_err(|error| format!("provider guide emitted invalid UTF-8: {error}"))?;
    let stdout = if forwarded.iter().any(|arg| arg == "--code") {
        stdout.to_string()
    } else {
        render_facade_guide(language_id, provider, stdout)
    };
    io::stdout()
        .write_all(stdout.as_bytes())
        .map_err(|error| format!("failed to write provider stdout: {error}"))
}

fn run_provider_process(
    language_id: &str,
    provider: &ActivatedProvider,
    program: &str,
    forwarded: &[String],
    project_root: &Path,
    cache_home: &Path,
) -> Result<ProviderProcessOutput, String> {
    run_provider_process_with_stdin(ProviderProcessRun {
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
        limits: default_provider_process_limits()?,
        stdin: StdinMode::Inherit,
    })
}

fn default_provider_process_limits() -> Result<ProviderProcessLimits, String> {
    let mut limits = ProviderProcessLimits::default();
    limits.timeout = provider_timeout_from_env()?;
    Ok(limits)
}

fn provider_timeout_from_env() -> Result<Option<Duration>, String> {
    let Ok(value) = env::var(ASP_PROVIDER_TIMEOUT_MS_ENV) else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let millis = trimmed.parse::<u64>().map_err(|error| {
        format!("{ASP_PROVIDER_TIMEOUT_MS_ENV} must be an integer number of milliseconds: {error}")
    })?;
    if millis == 0 {
        return Ok(None);
    }
    Ok(Some(Duration::from_millis(millis)))
}

struct ProviderProcessRun<'a> {
    language_id: &'a str,
    provider: &'a ActivatedProvider,
    program: &'a str,
    forwarded: &'a [String],
    project_root: &'a Path,
    cache_home: &'a Path,
    stdin: StdinMode,
    limits: ProviderProcessLimits,
}

fn run_provider_process_with_stdin(
    request: ProviderProcessRun<'_>,
) -> Result<ProviderProcessOutput, String> {
    let ProviderProcessRun {
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
        stdin,
        limits,
    } = request;
    let runtime_bin = project_state_paths(project_root)
        .map(|paths| paths.runtime_bin_dir)
        .unwrap_or_else(|_| runtime_bin_dir_for_cache_home(cache_home));
    let mut envs = BTreeMap::new();
    envs.insert(
        "PRJ_CACHE_HOME".to_string(),
        cache_home.to_string_lossy().to_string(),
    );
    envs.insert(
        "ASP_RUNTIME_BIN_DIR".to_string(),
        runtime_bin.to_string_lossy().to_string(),
    );
    if let Ok(protocol_bin) = env::current_exe() {
        envs.insert(
            "SEMANTIC_AGENT_PROTOCOL_BIN".to_string(),
            protocol_bin.to_string_lossy().to_string(),
        );
    }
    let mut path_entries = vec![runtime_bin.clone()];
    if let Some(path) = env::var_os("PATH") {
        path_entries.extend(env::split_paths(&path));
    }
    if let Some(home_local_bin) = home_local_bin_dir()
        && home_local_bin.is_dir()
    {
        path_entries.push(home_local_bin);
    }
    if let Ok(path) = env::join_paths(path_entries) {
        envs.insert("PATH".to_string(), path.to_string_lossy().to_string());
    }

    run_transport_process(ProviderProcessSpec {
        program: resolve_provider_program(program, project_root),
        args: forwarded.to_vec(),
        cwd: project_root.to_path_buf(),
        env: envs,
        stdin,
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits,
    })
    .map_err(|error| {
        format!(
            "failed to run provider `{}` for language `{language_id}`: {error}",
            provider.provider_id
        )
    })
}

fn home_local_bin_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".local/bin"))
}

fn resolve_provider_program(program: &str, project_root: &Path) -> String {
    let launch_cwd = env::current_dir().ok();
    resolve_provider_program_from(program, project_root, launch_cwd.as_deref())
}

fn resolve_provider_program_from(
    program: &str,
    project_root: &Path,
    launch_cwd: Option<&Path>,
) -> String {
    let program_path = Path::new(program);
    if program_path.is_absolute() || program_path.components().count() <= 1 {
        return program.to_string();
    }

    let candidates = launch_cwd
        .into_iter()
        .map(|cwd| cwd.join(program_path))
        .chain(std::iter::once(project_root.join(program_path)));

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        return candidate
            .canonicalize()
            .unwrap_or(candidate)
            .to_string_lossy()
            .to_string();
    }

    program.to_string()
}

pub(super) fn provider_invocation_with_profile(
    profiles: &RuntimeProfiles,
    provider: &ActivatedProvider,
    args: &[String],
    project_root: &Path,
    config: &AspConfig,
) -> Result<Vec<String>, String> {
    let mut invocation = provider_command_prefix(profiles, provider, project_root, config)?;
    invocation.extend(args.iter().cloned());
    Ok(invocation)
}

pub(super) fn provider_invocations(
    provider: &ActivatedProvider,
    args: &[String],
    project_root: &Path,
    profiles: &RuntimeProfiles,
    config: &AspConfig,
) -> Result<Vec<Vec<String>>, String> {
    search_scope_arg_sets(args, project_root)
        .into_iter()
        .map(|args| {
            provider_invocation_with_profile(profiles, provider, &args, project_root, config)
        })
        .collect()
}

fn provider_command_prefix(
    _profiles: &RuntimeProfiles,
    provider: &ActivatedProvider,
    project_root: &Path,
    config: &AspConfig,
) -> Result<Vec<String>, String> {
    let home = home_dir();
    if let Some(binary) = config.provider_bin(&provider.language_id) {
        return Ok(vec![resolve_configured_provider_binary(
            &provider.language_id,
            binary,
            project_root,
            home.as_deref(),
        )?]);
    }
    resolve_provider_binary_invocation(&provider.language_id, &provider.binary, home.as_deref())
        .map(|invocation| vec![invocation.command])
}

fn resolve_configured_provider_binary(
    language_id: &str,
    binary: &str,
    project_root: &Path,
    home: Option<&Path>,
) -> Result<String, String> {
    let binary_path = Path::new(binary);
    if binary_path.components().count() <= 1 {
        return resolve_provider_binary_invocation(language_id, binary, home)
            .map(|invocation| invocation.command);
    }
    Ok(resolve_provider_program(binary, project_root))
}

fn search_scope_arg_sets(args: &[String], project_root: &Path) -> Vec<Vec<String>> {
    if !is_search_scope_fanout_candidate(args) {
        return vec![args.to_vec()];
    }

    let mut scope_start = args.len();
    while scope_start > 0 && is_existing_directory_arg(project_root, &args[scope_start - 1]) {
        scope_start -= 1;
    }
    let scopes = &args[scope_start..];
    if scopes.len() <= 1 {
        return vec![args.to_vec()];
    }

    let prefix = &args[..scope_start];
    scopes
        .iter()
        .map(|scope| {
            let mut scoped = prefix.to_vec();
            scoped.push(scope.clone());
            scoped
        })
        .collect()
}

fn is_search_scope_fanout_candidate(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "search")
        && args.get(1).is_none_or(|subcommand| subcommand != "ingest")
}

fn is_existing_directory_arg(project_root: &Path, arg: &str) -> bool {
    if arg.starts_with('-') {
        return false;
    }
    let path = PathBuf::from(arg);
    let path = if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    };
    path.is_dir()
}

fn render_facade_guide(
    language_id: &str,
    provider: &ActivatedProvider,
    provider_stdout: &str,
) -> String {
    let mut lines = provider_stdout
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if let Some(command_line) = line.strip_prefix("|cmd ") {
                if let Some((prefix, command)) = command_line.split_once('=') {
                    format!(
                        "|cmd {prefix}={}",
                        rewrite_provider_command_mentions(language_id, provider, command)
                    )
                } else {
                    format!(
                        "|cmd {}",
                        rewrite_provider_command_mentions(language_id, provider, command_line)
                    )
                }
            } else if line.starts_with("[agent-guide] ") {
                rewrite_provider_command_mentions(
                    language_id,
                    provider,
                    &line
                        .replacen("[agent-guide]", "[guide]", 1)
                        .replace("protocol=agent-guide.v1", "protocol=guide.v1"),
                )
            } else if index == 0 && provider_specific_guide_header(line) {
                format!(
                    "[guide] lang={language_id} provider={} protocol=guide.v1 root=.",
                    provider.provider_id
                )
            } else if line == "|rule hook setup/runtime is owned by rs-harness" {
                "|rule hook setup/runtime is owned by semantic-agent-hook".to_string()
            } else {
                rewrite_provider_command_mentions(language_id, provider, line)
            }
        })
        .collect::<Vec<_>>();

    let v1_guide_contract = lines
        .first()
        .is_some_and(|line| line.contains("protocol=guide.v1"));
    let doctor_line =
        format!("|cmd agent-doctor=asp {language_id} agent doctor --workspace . --json");
    if !v1_guide_contract
        && !lines
            .iter()
            .any(|line| line.starts_with("|cmd agent-doctor="))
    {
        lines.push(doctor_line);
    }

    let mut output = lines.join("\n");
    if provider_stdout.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn write_facade_stream(
    language_id: &str,
    provider: &ActivatedProvider,
    bytes: &[u8],
    mut stream: impl Write,
) -> Result<(), String> {
    match std::str::from_utf8(bytes) {
        Ok(text) => stream
            .write_all(rewrite_provider_command_mentions(language_id, provider, text).as_bytes())
            .map_err(|error| format!("failed to write provider output: {error}")),
        Err(_) => stream
            .write_all(bytes)
            .map_err(|error| format!("failed to write provider output: {error}")),
    }
}

fn provider_specific_guide_header(line: &str) -> bool {
    line.starts_with('[') && line.contains("-guide]")
}

fn rewrite_provider_command_mentions(
    language_id: &str,
    provider: &ActivatedProvider,
    text: &str,
) -> String {
    let facade = format!("asp {language_id} ");
    text.replace(&format!("{} ", provider.binary), &facade)
}
