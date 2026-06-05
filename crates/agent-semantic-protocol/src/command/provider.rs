//! Language provider command facade.

use agent_semantic_hook::{
    ActivatedProvider, HookRuntime, default_activation_path, discover_activation_path,
    load_or_refresh_runtime_profiles, parse_hook_activation, runtime_profile_invocation,
    runtime_profiles_path_from_cache_home,
};
use std::env;
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SUPPORTED_LANGUAGES: &[&str] = &["rust", "typescript", "python"];
const SUPPORTED_COMMANDS: &[&str] = &[
    "search",
    "query",
    "check",
    "agent guide",
    "ast-patch",
    "evidence",
];

pub(crate) fn is_language_facade(language_id: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&language_id)
}

pub(crate) fn run_language_command(language_id: &str, args: &[String]) -> Result<(), String> {
    fn uses_client_backend(args: &[String]) -> bool {
        matches!(
            args.first().map(String::as_str),
            Some("search" | "query" | "check")
        )
    }

    fn activation_cache_home(activation_path: &Path) -> PathBuf {
        activation_storage_root(activation_path).join(".cache")
    }

    fn run_client_backend_command(
        language_id: &str,
        args: &[String],
        project_root: &Path,
        cache_home: &Path,
    ) -> Result<(), String> {
        let client_args = args.to_vec();
        let previous_cache_home = env::var_os("PRJ_HOME_CACHE");
        let previous_runtime_bin = env::var_os("ASP_RUNTIME_BIN_DIR");
        let previous_path = env::var_os("PATH");
        let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
        let mut path_entries = vec![runtime_bin.clone()];
        if let Some(path) = previous_path.as_deref() {
            path_entries.extend(env::split_paths(path));
        }
        let runtime_path = env::join_paths(path_entries).ok();
        unsafe {
            env::set_var("PRJ_HOME_CACHE", cache_home);
            env::set_var("ASP_RUNTIME_BIN_DIR", &runtime_bin);
            if let Some(path) = runtime_path.as_deref() {
                env::set_var("PATH", path);
            }
        }
        let result = agent_semantic_client::run_cli_args(
            Some(agent_semantic_client::LanguageId::from(language_id)),
            client_args,
            project_root.to_path_buf(),
        );
        match previous_cache_home {
            Some(value) => unsafe {
                env::set_var("PRJ_HOME_CACHE", value);
            },
            None => unsafe {
                env::remove_var("PRJ_HOME_CACHE");
            },
        }
        match previous_runtime_bin {
            Some(value) => unsafe {
                env::set_var("ASP_RUNTIME_BIN_DIR", value);
            },
            None => unsafe {
                env::remove_var("ASP_RUNTIME_BIN_DIR");
            },
        }
        match previous_path {
            Some(value) => unsafe {
                env::set_var("PATH", value);
            },
            None => unsafe {
                env::remove_var("PATH");
            },
        }
        result
    }

    if !is_language_facade(language_id) {
        return Err(language_usage());
    }
    validate_provider_command(args)?;

    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let activation_path = discover_activation_path(&invocation_root)
        .unwrap_or_else(|| default_activation_path(&invocation_root));
    let runtime = load_activation(&activation_path)?;
    let activation_root = activation_project_root(&activation_path, &runtime.project_root);
    let (project_root, provider_args) =
        effective_project_root_and_args(args, &invocation_root, &activation_root);

    let cache_home = activation_cache_home(&activation_path);
    if uses_client_backend(args) {
        return run_client_backend_command(language_id, &provider_args, &project_root, &cache_home);
    }

    let provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == language_id)
        .ok_or_else(|| format!("no activated provider for language {language_id}"))?;
    let runtime_profiles_path = runtime_profiles_path_from_cache_home(&cache_home);
    let runtime_profiles =
        load_or_refresh_runtime_profiles(&runtime_profiles_path, &project_root, &runtime)?;
    if is_agent_guide(args) {
        let invocation =
            provider_invocation_with_profile(&runtime_profiles, provider, &provider_args)?;
        return run_agent_guide_command(
            language_id,
            provider,
            &invocation,
            &project_root,
            &cache_home,
        );
    }
    for invocation in
        provider_invocations(provider, &provider_args, &project_root, &runtime_profiles)?
    {
        run_provider_command(
            language_id,
            provider,
            &invocation,
            &project_root,
            &cache_home,
        )?;
    }
    Ok(())
}

fn run_provider_command(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
) -> Result<(), String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
    let mut command = Command::new(program);
    command
        .args(forwarded)
        .current_dir(project_root)
        .env("PRJ_HOME_CACHE", cache_home)
        .env("ASP_RUNTIME_BIN_DIR", &runtime_bin)
        .stdin(Stdio::inherit());
    let mut path_entries = vec![runtime_bin];
    if let Some(path) = env::var_os("PATH") {
        path_entries.extend(env::split_paths(&path));
    }
    if let Ok(path) = env::join_paths(path_entries) {
        command.env("PATH", path);
    }
    let output = command.output().map_err(|error| {
        format!(
            "failed to spawn provider `{}` for language `{language_id}`: {error}",
            provider.provider_id
        )
    })?;
    write_facade_stream(language_id, provider, output.stderr, io::stderr())?;
    write_facade_stream(language_id, provider, output.stdout, io::stdout())?;
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }
    Ok(())
}

fn run_agent_guide_command(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
    cache_home: &Path,
) -> Result<(), String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
    let mut command = Command::new(program);
    command
        .args(forwarded)
        .current_dir(project_root)
        .env("PRJ_HOME_CACHE", cache_home)
        .env("ASP_RUNTIME_BIN_DIR", &runtime_bin);
    let mut path_entries = vec![runtime_bin];
    if let Some(path) = env::var_os("PATH") {
        path_entries.extend(env::split_paths(&path));
    }
    if let Ok(path) = env::join_paths(path_entries) {
        command.env("PATH", path);
    }
    let output = command.output().map_err(|error| {
        format!(
            "failed to spawn provider `{}` for language `{language_id}`: {error}",
            provider.provider_id
        )
    })?;
    io::stderr()
        .write_all(&output.stderr)
        .map_err(|error| format!("failed to write provider stderr: {error}"))?;
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("provider guide emitted invalid UTF-8: {error}"))?;
    let stdout = render_facade_guide(language_id, provider, &stdout);
    io::stdout()
        .write_all(stdout.as_bytes())
        .map_err(|error| format!("failed to write provider stdout: {error}"))
}

fn load_activation(path: &Path) -> Result<HookRuntime, String> {
    let text = fs::read_to_string(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            return format!(
                "[asp-provider] activation=missing path={}\n|reason provider-activation-missing\n|cmd install=asp hook install --client codex .\n|cmd guide=asp guide\n|cmd providers=asp providers",
                path.display()
            );
        }
        format!(
            "failed to read provider activation {}: {error}",
            path.display()
        )
    })?;
    parse_hook_activation(&text).map_err(|error| {
        format!(
            "failed to parse provider activation {}: {error:?}",
            path.display()
        )
    })
}

fn activation_project_root(activation_path: &Path, project_root: &str) -> PathBuf {
    let configured = PathBuf::from(project_root);
    if configured.is_absolute() {
        configured
    } else {
        activation_storage_root(activation_path).join(configured)
    }
}

fn effective_project_root_and_args(
    args: &[String],
    invocation_root: &Path,
    activation_root: &Path,
) -> (PathBuf, Vec<String>) {
    if let Some((root, args)) = explicit_positional_project_root(args, invocation_root) {
        return (root, args);
    }

    if invocation_root != activation_root
        && invocation_root.starts_with(activation_root)
        && invocation_root_is_provider_project(invocation_root)
    {
        return (invocation_root.to_path_buf(), args.to_vec());
    }

    if args.last().is_some_and(|arg| arg == ".")
        && invocation_root_is_provider_project(invocation_root)
    {
        (invocation_root.to_path_buf(), args.to_vec())
    } else {
        (activation_root.to_path_buf(), args.to_vec())
    }
}

fn explicit_positional_project_root(
    args: &[String],
    invocation_root: &Path,
) -> Option<(PathBuf, Vec<String>)> {
    let value = args.last()?;
    if value.starts_with('-') {
        return None;
    }
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        invocation_root.join(path)
    };
    if !positional_project_root_marker(&absolute) {
        return None;
    }
    let mut normalized_args = args.to_vec();
    if let Some(last) = normalized_args.last_mut() {
        *last = ".".to_string();
    }
    Some((absolute, normalized_args))
}

fn positional_project_root_marker(path: &Path) -> bool {
    path.join(".cache/agent-semantic-protocol/hooks/activation.json")
        .is_file()
        || path
            .join(".cache/agent-semantic-protocol/client/cache-manifest.json")
            .is_file()
        || invocation_root_is_provider_project(path)
        || path.join("Project.toml").is_file()
        || path.join("JuliaProject.toml").is_file()
}

fn invocation_root_is_provider_project(invocation_root: &Path) -> bool {
    invocation_root.join("Cargo.toml").is_file()
        || invocation_root.join("package.json").is_file()
        || invocation_root.join("pyproject.toml").is_file()
}

fn activation_storage_root(activation_path: &Path) -> PathBuf {
    activation_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn validate_provider_command(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(provider_usage());
    };
    let supported = if command == "agent" {
        args.get(1)
            .is_some_and(|subcommand| matches!(subcommand.as_str(), "guide" | "doctor"))
    } else {
        SUPPORTED_COMMANDS.contains(&command)
    };
    if supported {
        Ok(())
    } else {
        Err(provider_usage())
    }
}

fn is_agent_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "agent")
        && args.get(1).is_some_and(|subcommand| subcommand == "guide")
}

fn provider_invocation(provider: &ActivatedProvider, args: &[String]) -> Vec<String> {
    let mut invocation = if provider.provider_command_prefix.is_empty() {
        vec![provider.binary.clone()]
    } else {
        provider.provider_command_prefix.clone()
    };
    invocation.extend(args.iter().cloned());
    invocation
}

fn provider_invocation_with_profile(
    profiles: &agent_semantic_hook::RuntimeProfiles,
    provider: &ActivatedProvider,
    args: &[String],
) -> Result<Vec<String>, String> {
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

fn provider_invocations(
    provider: &ActivatedProvider,
    args: &[String],
    project_root: &Path,
    profiles: &agent_semantic_hook::RuntimeProfiles,
) -> Result<Vec<Vec<String>>, String> {
    search_scope_arg_sets(args, project_root)
        .into_iter()
        .map(|args| provider_invocation_with_profile(profiles, provider, &args))
        .collect()
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
        && !args.get(1).is_some_and(|subcommand| subcommand == "ingest")
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
        .map(|line| {
            if let Some((prefix, command)) = line
                .strip_prefix("|cmd ")
                .and_then(|line| line.split_once('='))
            {
                format!(
                    "|cmd {prefix}={}",
                    rewrite_provider_command_mentions(language_id, provider, command)
                )
            } else if line == "|rule hook install/runtime is owned by rs-harness" {
                "|rule hook install/runtime is owned by semantic-agent-hook".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>();

    let doctor_line = format!("|cmd agent-doctor=asp {language_id} agent doctor --json .");
    if !lines
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
    bytes: Vec<u8>,
    mut stream: impl Write,
) -> Result<(), String> {
    match String::from_utf8(bytes) {
        Ok(text) => stream
            .write_all(rewrite_provider_command_mentions(language_id, provider, &text).as_bytes())
            .map_err(|error| format!("failed to write provider output: {error}")),
        Err(error) => stream
            .write_all(&error.into_bytes())
            .map_err(|error| format!("failed to write provider output: {error}")),
    }
}

fn rewrite_provider_command_mentions(
    language_id: &str,
    provider: &ActivatedProvider,
    text: &str,
) -> String {
    let facade = format!("asp {language_id} ");
    text.replace(&format!("{} ", provider.binary), &facade)
}

fn runtime_profile_status_label(
    status: agent_semantic_hook::RuntimeProviderHealthStatus,
) -> &'static str {
    match status {
        agent_semantic_hook::RuntimeProviderHealthStatus::Available => "available",
        agent_semantic_hook::RuntimeProviderHealthStatus::Missing => "missing",
        agent_semantic_hook::RuntimeProviderHealthStatus::Unexecutable => "unexecutable",
    }
}

fn provider_usage() -> String {
    "usage: asp <rust|typescript|python> <search|query|check|agent guide|agent doctor|ast-patch|evidence> ..."
        .to_string()
}

fn language_usage() -> String {
    format!(
        "usage: asp <hook|ast-patch|graph|{}> ...",
        SUPPORTED_LANGUAGES.join("|")
    )
}
