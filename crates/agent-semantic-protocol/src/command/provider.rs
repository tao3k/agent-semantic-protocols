//! Language provider command facade.

use super::document_provider;
use agent_semantic_hook::{
    HookRuntime, default_activation_path, discover_activation_path, load_or_sync_activation,
    parse_hook_activation, runtime_profiles_for_runtime,
};
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use super::provider_process::{
    provider_invocation_with_profile, provider_invocations, run_guide_command, run_provider_command,
};
use super::provider_roots::{
    activation_project_root, activation_storage_root, client_backend_cache_home,
    effective_project_root_and_args,
};
use super::search_pipe::{is_asp_fast_search, run_asp_fast_search_command};

const SUPPORTED_LANGUAGES: &[&str] = &["rust", "typescript", "python", "julia", "org", "md"];
const SUPPORTED_COMMANDS: &[&str] = &["search", "query", "guide", "check", "ast-patch", "evidence"];

macro_rules! restore_env_var {
    ($name:literal, $previous:expr) => {
        match $previous {
            Some(value) => unsafe {
                env::set_var($name, value);
            },
            None => unsafe {
                env::remove_var($name);
            },
        }
    };
}

pub(crate) fn is_language_facade(language_id: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&language_id)
}

pub(crate) fn run_language_command(language_id: &str, args: &[String]) -> Result<(), String> {
    fn uses_client_backend(args: &[String]) -> bool {
        (args.first().is_some_and(|command| command == "search")
            && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
            || matches!(args.first().map(String::as_str), Some("check"))
            || (args.first().is_some_and(|command| command == "query")
                && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
    }

    fn run_client_backend_command(
        language_id: &str,
        args: &[String],
        project_root: &Path,
        cache_home: &Path,
    ) -> Result<(), String> {
        let client_args = args.to_vec();
        let previous_prj_cache_home = env::var_os("PRJ_CACHE_HOME");
        let previous_runtime_bin = env::var_os("ASP_RUNTIME_BIN_DIR");
        let previous_path = env::var_os("PATH");
        let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
        let mut path_entries = vec![runtime_bin.clone()];
        if let Some(path) = previous_path.as_deref() {
            path_entries.extend(env::split_paths(path));
        }
        let runtime_path = env::join_paths(path_entries).ok();
        unsafe {
            env::set_var("PRJ_CACHE_HOME", cache_home);
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
        restore_env_var!("PRJ_CACHE_HOME", previous_prj_cache_home);
        restore_env_var!("ASP_RUNTIME_BIN_DIR", previous_runtime_bin);
        restore_env_var!("PATH", previous_path);
        result
    }

    if !is_language_facade(language_id) {
        return Err(language_usage());
    }
    if document_provider::is_document_language(language_id) {
        return document_provider::run_language_command(language_id, args);
    }
    if is_help(args) {
        println!("{}", provider_usage());
        return Ok(());
    }
    validate_provider_command(args)?;
    if is_guide_help(args) {
        println!("{}", guide_usage(language_id));
        return Ok(());
    }

    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let activation_path = discover_activation_path(&invocation_root)
        .unwrap_or_else(|| default_activation_path(&invocation_root));
    let runtime = load_activation(&activation_path)?;
    let activation_root = activation_project_root(&activation_path, &runtime.project_root);
    let (project_root, provider_args) =
        effective_project_root_and_args(language_id, args, &invocation_root, &activation_root)?;

    let cache_home = client_backend_cache_home(&activation_root, &project_root)?;
    if is_asp_fast_search(&provider_args) {
        return run_asp_fast_search_command(language_id, &provider_args, &project_root);
    }
    let provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == language_id)
        .ok_or_else(|| format!("no activated provider for language {language_id}"))?;
    let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
    if uses_client_backend(args) {
        return run_client_backend_command(language_id, &provider_args, &project_root, &cache_home);
    }

    if is_guide(args) {
        let guide_args = provider_guide_args(language_id, &provider_args);
        let invocation =
            provider_invocation_with_profile(&runtime_profiles, provider, &guide_args)?;
        return run_guide_command(
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
    parse_hook_activation(&text).or_else(|error| {
        load_or_sync_activation(path, &activation_storage_root(path)).map_err(|sync_error| {
            format!(
                "failed to parse provider activation {}: {error:?}; failed to sync generated activation: {sync_error}",
                path.display()
            )
        })
    })
}

fn validate_provider_command(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(provider_usage());
    };
    let supported = if command == "agent" {
        args.get(1)
            .is_some_and(|subcommand| matches!(subcommand.as_str(), "doctor"))
    } else {
        SUPPORTED_COMMANDS.contains(&command)
    };
    if supported {
        Ok(())
    } else {
        Err(provider_usage())
    }
}

fn is_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "guide")
}

fn provider_guide_args(language_id: &str, args: &[String]) -> Vec<String> {
    if matches!(language_id, "python" | "typescript") && is_guide(args) {
        let mut rewritten = vec!["agent".to_string(), "guide".to_string()];
        rewritten.extend(args.iter().skip(1).cloned());
        rewritten
    } else {
        args.to_vec()
    }
}

fn is_help(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("--help" | "-h"))
}

fn is_guide_help(args: &[String]) -> bool {
    is_guide(args)
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
}

fn provider_usage() -> String {
    format!(
        "usage: asp <{}> [--help] <guide|search|query|check|agent doctor|ast-patch|evidence> ...",
        SUPPORTED_LANGUAGES.join("|")
    )
}

fn guide_usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} guide [--help] [PROJECT_ROOT]\n\nPrints the low-frequency provider-owned agent tool map.\nUse `asp {language_id} search guide .`, `asp {language_id} query guide .`, or `asp {language_id} query guide treesitter .` for focused reference guides."
    )
}

fn language_usage() -> String {
    format!(
        "usage: asp <hook|ast-patch|graph|{}> ...",
        SUPPORTED_LANGUAGES.join("|")
    )
}
