use agent_semantic_hook::{
    ActivatedProvider, RuntimeProfiles, RuntimeProviderHealthStatus, runtime_profile_invocation,
};
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessOutput, ProviderProcessSpec, StdinMode,
    run_provider_process as run_transport_process,
};
use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::search_config::AspConfig;

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

pub(super) fn run_provider_command_with_stdin(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
    stdin: Vec<u8>,
) -> Result<ProviderProcessOutput, String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    run_provider_process_with_stdin(
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
        StdinMode::bytes(stdin),
    )
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
    let stdout = render_facade_guide(language_id, provider, stdout);
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
    run_provider_process_with_stdin(
        language_id,
        provider,
        program,
        forwarded,
        project_root,
        cache_home,
        StdinMode::Inherit,
    )
}

fn run_provider_process_with_stdin(
    language_id: &str,
    provider: &ActivatedProvider,
    program: &str,
    forwarded: &[String],
    project_root: &Path,
    cache_home: &Path,
    stdin: StdinMode,
) -> Result<ProviderProcessOutput, String> {
    let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
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
    let mut path_entries = vec![runtime_bin];
    if let Some(path) = env::var_os("PATH") {
        path_entries.extend(env::split_paths(&path));
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
        limits: ProviderProcessLimits::default(),
    })
    .map_err(|error| {
        format!(
            "failed to run provider `{}` for language `{language_id}`: {error}",
            provider.provider_id
        )
    })
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
    config: &AspConfig,
) -> Result<Vec<String>, String> {
    if let Some(binary) = config.provider_bin(&provider.language_id) {
        return Ok(provider_invocation_with_binary(provider, args, binary));
    }
    if let Some(invocation) = runtime_profile_invocation(profiles, provider, args) {
        return Ok(invocation);
    }
    if let Some(profile) = profiles.providers.iter().find(|profile| {
        profile.manifest_id == provider.manifest_id
            && profile.language_id == provider.language_id
            && profile.provider_id == provider.provider_id
            && profile.binary == provider.binary
    }) {
        return Err(format!(
            "runtime profile for provider `{}` language `{}` is {}; run `asp hook doctor --client codex .`",
            provider.provider_id,
            provider.language_id,
            runtime_profile_status_label(profile.health.status)
        ));
    }
    Ok(provider_invocation(provider, args))
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
        .map(|args| provider_invocation_with_profile(profiles, provider, &args, config))
        .collect()
}

fn provider_invocation(provider: &ActivatedProvider, args: &[String]) -> Vec<String> {
    provider_invocation_with_binary(provider, args, &provider.binary)
}

fn provider_invocation_with_binary(
    provider: &ActivatedProvider,
    args: &[String],
    binary: &str,
) -> Vec<String> {
    let mut invocation = if provider.provider_command_prefix.is_empty() {
        vec![binary.to_string()]
    } else {
        let mut prefix = provider.provider_command_prefix.clone();
        if let Some(program) = prefix.first_mut() {
            *program = binary.to_string();
        }
        prefix
    };
    invocation.extend(args.iter().cloned());
    invocation
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
            } else if line == "|rule hook install/runtime is owned by rs-harness" {
                "|rule hook install/runtime is owned by semantic-agent-hook".to_string()
            } else {
                rewrite_provider_command_mentions(language_id, provider, line)
            }
        })
        .collect::<Vec<_>>();

    let v1_guide_contract = lines
        .first()
        .is_some_and(|line| line.contains("protocol=guide.v1"));
    let doctor_line = format!("|cmd agent-doctor=asp {language_id} agent doctor --json .");
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

fn runtime_profile_status_label(status: RuntimeProviderHealthStatus) -> &'static str {
    match status {
        RuntimeProviderHealthStatus::Available => "available",
        RuntimeProviderHealthStatus::Missing => "missing",
        RuntimeProviderHealthStatus::Unexecutable => "unexecutable",
    }
}
