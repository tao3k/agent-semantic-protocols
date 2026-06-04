use serde_json::Value;

use super::aliases::GraphAlias;

pub(super) fn graph_profiles_line(packet: &Value, aliases: &[GraphAlias]) -> Option<String> {
    let profile_entries = packet
        .get("reasoningProfiles")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|profile| graph_profile_entry(profile, aliases))
        .collect::<Vec<_>>();
    (!profile_entries.is_empty()).then(|| format!("profiles={}", profile_entries.join(",")))
}

fn graph_profile_entry(profile: &Value, aliases: &[GraphAlias]) -> Option<String> {
    let profile_name = compact_profile_atom(profile.get("profile")?.as_str()?)?;
    let handles = profile
        .get("compatibleHandles")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .filter_map(compact_alias_handle)
        .collect::<Vec<_>>();
    if handles.is_empty()
        || !handles
            .iter()
            .all(|handle| aliases.iter().any(|alias| alias.id == *handle))
    {
        return None;
    }
    Some(format!("{}({})", profile_name, handles.join(",")))
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
