use serde_json::Value;

use super::aliases::GraphAlias;

pub(super) fn graph_profiles_line(packet: &Value, aliases: &[GraphAlias]) -> Option<String> {
    fn selected_reasoning_profile(packet: &Value) -> Option<String> {
        let header_profile = packet
            .get("profile")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let header_query_profile = packet
            .get("q")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        header_profile
            .or(header_query_profile)
            .map(ToOwned::to_owned)
    }

    let selected_profile = selected_reasoning_profile(packet);
    let entries = packet
        .get("reasoningProfiles")
        .and_then(Value::as_array)?
        .iter()
        .filter(|profile| match selected_profile.as_deref() {
            Some(selected) => profile.get("profile").and_then(Value::as_str) == Some(selected),
            None => true,
        })
        .filter_map(|profile| graph_profile_entry(profile, aliases))
        .collect::<Vec<_>>();
    (!entries.is_empty()).then(|| format!("entries={}", entries.join(",")))
}

fn graph_profile_entry(profile: &Value, aliases: &[GraphAlias]) -> Option<String> {
    let profile_name = compact_profile_atom(profile.get("profile")?.as_str()?)?;
    let mut handles = Vec::new();
    for selector in profile.get("selectors")?.as_array()? {
        let alias = compact_alias_handle(selector.get("alias")?.as_str()?)?;
        let selector_kind = compact_profile_atom(selector.get("kind")?.as_str()?)?;
        let required = selector
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let present = aliases
            .iter()
            .any(|graph_alias| graph_alias.id == alias && graph_alias.node_type == selector_kind);
        if present {
            handles.push(alias.to_string());
        } else if required {
            return None;
        }
    }
    if handles.is_empty() {
        return None;
    }
    let returns = profile
        .get("returns")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter_map(compact_profile_atom)
        .collect::<Vec<_>>();
    if returns.is_empty() {
        return None;
    }
    Some(format!(
        "{}({}=>{})",
        profile_name,
        handles.join(","),
        returns.join("+")
    ))
}

fn compact_profile_atom(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_'))
    .then_some(value)
}

fn compact_alias_handle(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit()))
    .then_some(value)
}
