//! Thin root `asp search` / `asp query` router over language facades.

use std::path::Path;

use super::provider_dispatch::{
    is_language_facade, run_language_command, unsupported_language_facade_message,
};

pub(crate) async fn run_root_language_facade(command: &str, args: &[String]) -> Result<(), String> {
    let (language_id, provider_args) = root_language_and_args(command, args)?;
    let mut language_args = vec![command.to_string()];
    language_args.extend(provider_args);
    run_language_command(&language_id, &language_args, tokio::time::Instant::now()).await
}

fn root_language_and_args(command: &str, args: &[String]) -> Result<(String, Vec<String>), String> {
    let (explicit_language, provider_args) = split_root_language_arg(command, args)?;
    let provider_args = normalize_root_provider_args(command, provider_args);
    let profiles = agent_semantic_config::default_hook_client_config_file()?
        .profiles
        .into_values()
        .map(|profile| (profile.language_id, profile.extension_any))
        .collect::<Vec<_>>();
    if let Some(language) = explicit_language {
        if is_language_facade(&language) {
            return Ok((language, provider_args));
        }
        return Err(unsupported_language_facade_message(
            &language,
            Some(command),
            None,
        ));
    }

    infer_root_facade_language(&provider_args, &profiles)?
        .map(|language| (language, provider_args))
        .ok_or_else(|| root_facade_language_required(command, &profiles))
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

fn infer_root_facade_language(
    args: &[String],
    profiles: &[(String, Vec<String>)],
) -> Result<Option<String>, String> {
    let path_languages = args
        .iter()
        .filter_map(|arg| language_from_path_like_arg(arg, profiles))
        .collect::<std::collections::BTreeSet<_>>();
    if path_languages.len() == 1 {
        return Ok(path_languages.into_iter().next().map(str::to_string));
    }
    Ok(None)
}

fn language_from_path_like_arg<'a>(
    arg: &str,
    profiles: &'a [(String, Vec<String>)],
) -> Option<&'a str> {
    let path = selector_path(arg);
    let extension = Path::new(path).extension()?.to_str()?;
    let matches = profiles
        .iter()
        .filter(|(_language_id, extensions)| {
            extensions
                .iter()
                .any(|source_extension| source_extension.trim_start_matches('.') == extension)
        })
        .map(|(language_id, _extensions)| language_id.as_str())
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

fn root_facade_language_required(command: &str, profiles: &[(String, Vec<String>)]) -> String {
    let languages = profiles
        .iter()
        .map(|(language_id, _extensions)| language_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("|");
    let languages = if languages.is_empty() {
        "language"
    } else {
        languages.as_str()
    };
    format!(
        "asp {command} requires --language <{languages}> or an unambiguous path/selector; use `asp {command} --language <language> ...` or `asp <language> {command} ...`"
    )
}
