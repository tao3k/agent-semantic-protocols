// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Registered language facade selector backed by the provider protocol registry.

fn registered_language_facades() -> Vec<String> {
    agent_semantic_provider_protocol::builtin_provider_registrations()
        .map(|registrations| {
            registrations
                .into_iter()
                .map(|registration| registration.language_id)
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
) -> String {
    let known_facades = registered_language_facades_line();
    let suggested_facade = suggested_language_facade_for_request(requested_facade, &known_facades);
    let mut lines = vec![
        format!("unsupported ASP language facade `{requested_facade}`."),
        "ASP facades are language IDs, not package or library names.".to_string(),
        format!("Known language facades: {known_facades}."),
    ];
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
        "For unsupported target-language files, use provider-neutral search or install/activate a matching provider.".to_string(),
    ]);
    lines.join("\n")
}

fn suggested_language_facade_for_request(
    requested_facade: &str,
    known_facades: &str,
) -> Option<String> {
    if requested_facade.eq_ignore_ascii_case("effect")
        && known_facades
            .split('|')
            .any(|facade| facade == "typescript")
    {
        return Some("typescript".to_string());
    }
    None
}
