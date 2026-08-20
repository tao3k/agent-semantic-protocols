pub(crate) struct HookPolicyCandidate {
    pub(crate) priority: i64,
    pub(crate) terminal: bool,
    pub(crate) decision: crate::HookDecision,
}
