//! Binds one physical plugin matcher to an immutable Host invocation fact.

use serde_json::Value;

use crate::action_ir::HostInvocationKind;

const HOST_MATCHER_SIGNAL_FIELD: &str = "_aspHostMatcher";

fn host_invocation_from_matcher(matcher: &str) -> Option<HostInvocationKind> {
    match matcher {
        "Read" => Some(HostInvocationKind::Read),
        "apply_patch" | "Write" | "Edit" | "NotebookEdit" => Some(HostInvocationKind::Edit),
        "Bash" => Some(HostInvocationKind::Execute),
        "spawn_agent" => Some(HostInvocationKind::SpawnAgent),
        "prefix:mcp__" => Some(HostInvocationKind::Mcp),
        _ => None,
    }
}

/// Binds the physical plugin matcher to the Host payload before classification.
///
/// Exact matchers compare the real tool name; family matchers compare one
/// declared prefix. This validates Host identity and does not classify shell
/// semantics.
pub fn bind_plugin_host_matcher(
    payload: &mut Value,
    exact_matcher: Option<&str>,
    matcher_prefix: Option<&str>,
) -> Result<(), String> {
    if exact_matcher.is_some() == matcher_prefix.is_some() {
        return Err(
            "plugin Host matcher requires exactly one --host-match or --host-match-prefix"
                .to_owned(),
        );
    }
    let matcher = exact_matcher
        .map(str::to_owned)
        .unwrap_or_else(|| format!("prefix:{}", matcher_prefix.unwrap_or_default()));
    if host_invocation_from_matcher(&matcher).is_none() {
        return Err(format!("unknown plugin Host matcher `{matcher}`"));
    }
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "plugin Host matcher payload requires tool_name".to_owned())?;
    let matcher_matches = exact_matcher.is_some_and(|expected| tool_name == expected)
        || matcher_prefix.is_some_and(|prefix| tool_name.starts_with(prefix));
    if !matcher_matches {
        return Err(format!(
            "plugin Host matcher binding mismatch: matcher={matcher} toolName={tool_name}"
        ));
    }
    let object = payload
        .as_object_mut()
        .ok_or_else(|| "plugin Host matcher payload must be an object".to_owned())?;
    object.insert(HOST_MATCHER_SIGNAL_FIELD.to_owned(), Value::String(matcher));
    Ok(())
}

pub(crate) fn plugin_host_action(payload: &Value) -> Option<HostInvocationKind> {
    payload
        .get(HOST_MATCHER_SIGNAL_FIELD)
        .and_then(Value::as_str)
        .and_then(host_invocation_from_matcher)
}
