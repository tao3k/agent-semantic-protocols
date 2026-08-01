use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind, render_platform_response,
};
use std::collections::BTreeMap;

pub(super) fn emit_decision(emit: &str, decision: &HookDecision) -> Result<(), String> {
    let output_value = match emit {
        "decision" => serde_json::to_value(decision)
            .map_err(|error| format!("failed to serialize hook decision: {error}"))?,
        "platform" => render_platform_response(decision)
            .map_err(|error| format!("failed to render hook response: {error:?}"))?,
        other => {
            return Err(format!(
                "unsupported --emit value: {other}; expected platform or decision"
            ));
        }
    };
    let output = serde_json::to_string(&output_value)
        .map_err(|error| format!("failed to serialize hook response: {error}"))?;
    let captured = HOOK_OUTPUT_CAPTURE.with(|capture| {
        let mut capture = capture.borrow_mut();
        if let Some(buffer) = capture.as_mut() {
            *buffer = output.clone();
            true
        } else {
            false
        }
    });
    if !captured {
        println!("{output}");
    }
    Ok(())
}

pub(super) fn emit_hook_runtime_failure(
    client: &str,
    event: &str,
    emit: &str,
    message: &str,
) -> Result<(), String> {
    let decision = HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: client.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: message.to_string(),
        fields: BTreeMap::new(),
    };
    emit_decision(emit, &decision)
}
use std::cell::RefCell;

thread_local! {
    static HOOK_OUTPUT_CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(super) fn with_hook_output_capture<T>(
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<(T, String), String> {
    HOOK_OUTPUT_CAPTURE.with(|capture| {
        if capture.borrow().is_some() {
            return Err("nested hook output capture is not supported".to_owned());
        }
        *capture.borrow_mut() = Some(String::new());
        let result = operation();
        let output = capture.borrow_mut().take().unwrap_or_default();
        result.map(|value| (value, output))
    })
}
