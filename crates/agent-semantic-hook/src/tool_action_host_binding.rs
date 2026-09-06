//! Binds one physical plugin matcher to an immutable Host invocation fact.

use serde_json::Value;

use crate::action_ir::HostInvocationKind;

const HOST_MATCHER_SIGNAL_FIELD: &str = "_aspHostMatcher";

fn host_invocation_from_matcher(matcher: &str) -> Option<HostInvocationKind> {
    match matcher {
        "apply_patch" => Some(HostInvocationKind::Edit),
        "Bash" => Some(HostInvocationKind::Execute),
        "Read" => Some(HostInvocationKind::Read),
        "spawn_agent" => Some(HostInvocationKind::SpawnAgent),
        matcher if matcher.starts_with("mcp__") => Some(HostInvocationKind::Mcp),
        _ => None,
    }
}

/// Binds the physical plugin matcher to the Host payload before classification.
///
/// Exact matchers compare the real tool name; family matchers compare one
/// declared prefix. This validates Host identity and does not classify shell
/// semantics.
pub fn bind_plugin_host_matcher(payload: &mut Value, exact_matcher: &str) -> Result<(), String> {
    if exact_matcher.trim().is_empty() {
        return Err("plugin Host matcher requires one non-empty --host-match".to_owned());
    }
    if host_invocation_from_matcher(exact_matcher).is_none() {
        return Err(format!("unknown plugin Host matcher `{exact_matcher}`"));
    }
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "plugin Host matcher payload requires tool_name".to_owned())?;
    if tool_name != exact_matcher {
        return Err(format!(
            "plugin Host matcher binding mismatch: matcher={exact_matcher} toolName={tool_name}"
        ));
    }
    let object = payload
        .as_object_mut()
        .ok_or_else(|| "plugin Host matcher payload must be an object".to_owned())?;
    object.insert(
        HOST_MATCHER_SIGNAL_FIELD.to_owned(),
        Value::String(exact_matcher.to_owned()),
    );
    Ok(())
}

pub(crate) fn plugin_host_action(payload: &Value) -> Option<HostInvocationKind> {
    payload
        .get(HOST_MATCHER_SIGNAL_FIELD)
        .and_then(Value::as_str)
        .and_then(host_invocation_from_matcher)
}
