use agent_semantic_hook::HookRuntime;

fn registered_language_facades() -> Vec<String> {
    agent_semantic_config::default_hook_client_config_file()
        .map(|config| {
            config
                .profiles
                .values()
                .map(|profile| profile.language_id.clone())
                .filter(|language_id| !matches!(language_id.as_str(), "md" | "org"))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn registered_language_facades_line() -> String {
    registered_language_facades().join("|")
}

pub(crate) fn is_language_facade(language_id: &str) -> bool {
    registered_language_facades()
        .iter()
        .any(|facade| facade == language_id)
}

pub(crate) fn unsupported_language_facade_message(
    requested_facade: &str,
    command: Option<&str>,
    runtime: Option<&HookRuntime>,
) -> String {
    let known_facades = registered_language_facades_line();
    let active_facades = runtime
        .map(active_language_facades)
        .filter(|facades| !facades.is_empty());
    let suggested_facade = runtime.and_then(|runtime| {
        suggested_language_facade_for_request(requested_facade, &active_language_facades(runtime))
    });
    let mut lines = vec![
        format!("unsupported ASP language facade `{requested_facade}`."),
        "ASP facades are language IDs, not package or library names.".to_string(),
        format!("Known language facades: {known_facades}."),
    ];
    if let Some(active_facades) = active_facades.as_deref() {
        lines.push(format!("Active language facades: {active_facades}."));
    }
    if let Some(suggested_facade) = suggested_facade.as_deref() {
        lines.push(format!("Suggested matching facade: {suggested_facade}."));
    }
    lines.extend([String::new(), "## Run Next".to_string()]);
    if let Some(suggested_facade) = suggested_facade.as_deref() {
        let command = command.unwrap_or("guide");
        lines.push(format!("asp {suggested_facade} {command} ..."));
    } else {
        lines.push("asp providers".to_string());
    }
    lines.extend([
        String::new(),
        "## Rules".to_string(),
        "Only run `asp <language> search|query` when the facade is listed and matches the target language.".to_string(),
        "Do not switch to an unrelated active facade just because it is the only provider in this repository.".to_string(),
        "For unsupported target-language files, use provider-neutral finder commands or install/activate a matching provider.".to_string(),
    ]);
    lines.join("\n")
}

fn active_language_facades(runtime: &HookRuntime) -> String {
    runtime
        .providers
        .iter()
        .map(|provider| provider.language_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("|")
}

fn suggested_language_facade_for_request(
    requested_facade: &str,
    active_facades: &str,
) -> Option<String> {
    if requested_facade.eq_ignore_ascii_case("effect")
        && active_facades
            .split('|')
            .any(|facade| facade == "typescript")
    {
        return Some("typescript".to_string());
    }
    None
}

pub(super) fn is_runtime_exact_query(args: &[String]) -> bool {
    if !matches!(args.first().map(String::as_str), Some("query"))
        || args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--term" | "--treesitter-query"))
    {
        return false;
    }
    exact_query_selector_argument(args).is_some()
}

pub(super) fn exact_query_selector_argument(args: &[String]) -> Option<&str> {
    option_value(args, "--selector").or_else(|| {
        args.get(1)
            .filter(|selector| !selector.starts_with('-'))
            .map(String::as_str)
    })
}

pub(super) fn option_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix))
        .or_else(|| {
            args.windows(2)
                .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
        })
}
