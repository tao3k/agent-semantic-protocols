use std::collections::BTreeMap;

use crate::DecisionKind;
use crate::DecisionSubject;
use crate::HOOK_DECISION_SCHEMA_ID;
use crate::HOOK_DECISION_SCHEMA_VERSION;
use crate::HOOK_PROTOCOL_ID;
use crate::HOOK_PROTOCOL_VERSION;
use crate::HookDecision;
use crate::ReasonKind;

pub(super) fn allow(platform: &str, event: &str, subject: DecisionSubject) -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_owned(),
        event: event.to_owned(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject,
        routes: Vec::new(),
        message: "Allowed by semantic agent hook runtime.".to_owned(),
        fields: BTreeMap::new(),
    }
}
