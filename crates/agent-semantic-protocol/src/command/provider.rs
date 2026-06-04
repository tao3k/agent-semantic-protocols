//! Language provider command facade.

use agent_semantic_hook::{
    ActivatedProvider, HookRuntime, default_activation_path, discover_activation_path,
    parse_hook_activation,
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
    if !is_language_facade(language_id) {
        return Err(language_usage());
    }
    validate_provider_command(args)?;

    let invocation_root = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    let activation_path = discover_activation_path(&invocation_root)
        .unwrap_or_else(|| default_activation_path(&invocation_root));
    let runtime = load_activation(&activation_path)?;
    let project_root = activation_project_root(&activation_path, &runtime.project_root);
    let provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == language_id)
        .ok_or_else(|| {
            format!(
                "activation {} does not include language `{language_id}`; run `asp hook install --client codex .` from the project root",
                activation_path.display()
            )
        })?;
    let invocation = provider_invocation(provider, args);
    if is_agent_guide(args) {
        return run_agent_guide_command(language_id, provider, &invocation, &project_root);
    }
    run_provider_command(language_id, provider, &invocation, &project_root)
}

fn run_provider_command(
    language_id: &str,
    provider: &ActivatedProvider,
    invocation: &[String],
    project_root: &Path,
) -> Result<(), String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    let output = Command::new(program)
        .args(forwarded)
        .current_dir(&project_root)
        .stdin(Stdio::inherit())
        .output()
        .map_err(|error| {
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
) -> Result<(), String> {
    let (program, forwarded) = invocation
        .split_first()
        .ok_or_else(|| format!("language `{language_id}` has an empty provider command"))?;
    let output = Command::new(program)
        .args(forwarded)
        .current_dir(project_root)
        .output()
        .map_err(|error| {
            format!(
                "failed to spawn provider `{}` for language `{language_id}`: {error}",
                provider.provider_id
            )
        })?;
    io::stderr()
        .write_all(&output.stderr)
        .map_err(|error| format!("failed to write provider guide stderr: {error}"))?;
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("provider guide emitted non-UTF-8 output: {error}"))?;
    let stdout = render_facade_guide(language_id, provider, &stdout);
    io::stdout()
        .write_all(stdout.as_bytes())
        .map_err(|error| format!("failed to write provider guide stdout: {error}"))
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
    let supported = match command {
        "search" | "query" | "check" | "ast-patch" | "evidence" => true,
        "agent" => args.get(1).is_some_and(|subcommand| subcommand == "guide"),
        _ => false,
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

fn render_facade_guide(
    language_id: &str,
    provider: &ActivatedProvider,
    provider_stdout: &str,
) -> String {
    provider_stdout
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
            } else if line == "|rule hook install/runtime is owned by agent-semantic-hook" {
                "|rule hook install/runtime uses asp hook; agent-semantic-hook owns classification runtime"
                    .to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + provider_stdout
            .ends_with('\n')
            .then_some("\n")
            .unwrap_or("")
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

fn provider_usage() -> String {
    format!(
        "usage: asp <{}> <{}> ...",
        SUPPORTED_LANGUAGES.join("|"),
        SUPPORTED_COMMANDS.join("|")
    )
}

fn language_usage() -> String {
    format!(
        "usage: asp <hook|ast-patch|graph|{}> ...",
        SUPPORTED_LANGUAGES.join("|")
    )
}
