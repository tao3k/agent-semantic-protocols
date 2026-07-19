use agent_semantic_hook::HookRuntime;

fn registered_language_facades() -> Vec<String> {
    let mut facades = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .map(|manifest| manifest.language_id.to_string())
        .collect::<Vec<_>>();
    facades.sort();
    facades.dedup();
    facades
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
        lines.extend([
            "asp providers".to_string(),
            "asp fd -query '<path-or-language-term>' '.'".to_string(),
            "asp rg -query '<feature-term>' '<bounded-scope>'".to_string(),
        ]);
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
