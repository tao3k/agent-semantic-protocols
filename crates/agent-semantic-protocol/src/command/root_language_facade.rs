//! Thin root `asp search` / `asp query` router over language facades.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_hook::{
    HookRuntime, default_activation_path, discover_activation_path, parse_hook_activation,
};

use super::provider_dispatch::{
    is_language_facade, run_language_command, unsupported_language_facade_message,
};

pub(crate) fn run_root_language_facade(command: &str, args: &[String]) -> Result<(), String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    let (language_id, provider_args) = root_language_and_args(command, args, &cwd)?;
    let mut language_args = vec![command.to_string()];
    language_args.extend(provider_args);
    run_language_command(&language_id, &language_args)
}

fn root_language_and_args(
    command: &str,
    args: &[String],
    cwd: &Path,
) -> Result<(String, Vec<String>), String> {
    let (explicit_language, provider_args) = split_root_language_arg(command, args)?;
    let provider_args = normalize_root_provider_args(command, provider_args);
    let runtime = load_activation_runtime(cwd);
    if let Some(language) = explicit_language {
        if is_language_facade(&language) {
            return Ok((language, provider_args));
        }
        return Err(unsupported_language_facade_message(
            &language,
            Some(command),
            runtime.as_ref(),
        ));
    }

    infer_root_facade_language(&provider_args, cwd, runtime.as_ref())
        .map(|language| (language, provider_args))
        .ok_or_else(|| root_facade_language_required(command, runtime.as_ref()))
}

fn split_root_language_arg(
    command: &str,
    args: &[String],
) -> Result<(Option<String>, Vec<String>), String> {
    let mut provider_args = Vec::new();
    let mut explicit_language = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--language" || arg == "-l" {
            let language = args
                .get(index + 1)
                .ok_or_else(|| format!("asp {command} --language requires a language id"))?;
            explicit_language = Some(language.clone());
            index += 2;
        } else if let Some(language) = arg.strip_prefix("--language=") {
            explicit_language = Some(language.to_string());
            index += 1;
        } else {
            provider_args.push(arg.clone());
            index += 1;
        }
    }
    Ok((explicit_language, provider_args))
}

fn normalize_root_provider_args(command: &str, args: Vec<String>) -> Vec<String> {
    if command == "query" {
        strip_query_view_seeds(args)
    } else {
        args
    }
}

fn strip_query_view_seeds(args: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--view" && args.get(index + 1).is_some_and(|value| value == "seeds") {
            index += 2;
        } else if arg == "--view=seeds" {
            index += 1;
        } else {
            normalized.push(arg.clone());
            index += 1;
        }
    }
    normalized
}

fn load_activation_runtime(cwd: &Path) -> Option<HookRuntime> {
    let activation_path =
        discover_activation_path(cwd).unwrap_or_else(|| default_activation_path(cwd));
    let text = fs::read_to_string(activation_path).ok()?;
    parse_hook_activation(&text).ok()
}

fn infer_root_facade_language(
    args: &[String],
    cwd: &Path,
    runtime: Option<&HookRuntime>,
) -> Option<String> {
    let runtime = runtime?;
    let path_languages = args
        .iter()
        .filter_map(|arg| language_from_path_like_arg(arg, runtime))
        .collect::<std::collections::BTreeSet<_>>();
    if path_languages.len() == 1 {
        return path_languages.into_iter().next().map(str::to_string);
    }

    let project_roots = args
        .iter()
        .filter_map(|arg| project_root_candidate(arg, cwd))
        .collect::<Vec<_>>();
    let marker_languages = project_roots
        .iter()
        .flat_map(|root| marker_languages(root, runtime))
        .collect::<std::collections::BTreeSet<_>>();
    if marker_languages.len() == 1 {
        return marker_languages.into_iter().next().map(str::to_string);
    }
    None
}

fn language_from_path_like_arg<'a>(arg: &str, runtime: &'a HookRuntime) -> Option<&'a str> {
    let path = selector_path(arg);
    let extension = Path::new(path).extension()?.to_str()?;
    let matches = runtime
        .providers
        .iter()
        .filter(|provider| {
            provider
                .source_extensions
                .iter()
                .any(|source_extension| source_extension.trim_start_matches('.') == extension)
        })
        .map(|provider| provider.language_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    (matches.len() == 1).then(|| *matches.first().expect("one language match"))
}

fn selector_path(arg: &str) -> &str {
    let Some((path, rest)) = arg.split_once(':') else {
        return arg;
    };
    if rest
        .split(':')
        .next()
        .is_some_and(|part| part.chars().all(|character| character.is_ascii_digit()))
    {
        path
    } else {
        arg
    }
}

fn project_root_candidate(arg: &str, cwd: &Path) -> Option<PathBuf> {
    if arg.starts_with('-') || arg.contains(':') {
        return None;
    }
    let path = if Path::new(arg).is_absolute() {
        PathBuf::from(arg)
    } else {
        cwd.join(arg)
    };
    path.is_dir().then_some(path)
}

fn marker_languages<'a>(root: &Path, runtime: &'a HookRuntime) -> Vec<&'a str> {
    runtime
        .providers
        .iter()
        .filter(|provider| {
            provider
                .config_files
                .iter()
                .any(|config_file| root.join(config_file).exists())
        })
        .map(|provider| provider.language_id.as_str())
        .collect()
}

fn root_facade_language_required(command: &str, runtime: Option<&HookRuntime>) -> String {
    let languages = runtime
        .map(active_language_hint)
        .filter(|languages| !languages.is_empty())
        .unwrap_or_else(|| "language".to_string());
    format!(
        "asp {command} requires --language <{languages}> or an unambiguous path/selector; use `asp {command} --language <language> ...` or `asp <language> {command} ...`"
    )
}

fn active_language_hint(runtime: &HookRuntime) -> String {
    runtime
        .providers
        .iter()
        .map(|provider| provider.language_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("|")
}
