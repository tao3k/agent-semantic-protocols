//! Host action text for configured resident-agent lifecycle menus.

pub(super) fn managed_agent_kind(name: &str) -> std::borrow::Cow<'_, str> {
    if name.is_empty()
        || name.starts_with('-')
        || name.ends_with('-')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return std::borrow::Cow::Borrowed("unknown-resident-agent-kind");
    }

    match name {
        "asp-explore" => std::borrow::Cow::Borrowed("asp_explorer"),
        "asp-testing" => std::borrow::Cow::Borrowed("asp_testing"),
        _ => std::borrow::Cow::Owned(name.replace('-', "_")),
    }
}

pub(super) fn host_tree_audit_action(name: &str) -> &'static str {
    if name == "asp-testing" {
        "Call the native collaboration list-agents surface for this root task. If /root/asp_testing is absent, record `direnv exec . asp agent session observe-host-tree --name asp-testing --resident-target-status absent` and re-enter bootstrap. If it is present, record the corresponding present observation with canonical target /root/asp_testing before attempting follow-up. A historical rollout ID alone is never a callable target."
    } else {
        "Call the native collaboration list-agents surface for this root task. If /root/asp_explorer is absent, record `direnv exec . asp agent session observe-host-tree --name asp-explore --resident-target-status absent` and re-enter bootstrap. If it is present, record the corresponding present observation before attempting follow-up. A historical rollout ID alone is never a callable target."
    }
}

pub(super) fn host_tree_resume_action(name: &str) -> String {
    let canonical_target = format!("/root/{}", managed_agent_kind(name));
    format!(
        "Use the main agent's native follow-up surface for {canonical_target}. Completed or idle native children remain resumable and must keep the same identity. If the follow-up succeeds, immediately record `direnv exec . asp agent session observe-host-ack --name {name} --canonical-target {canonical_target} --evidence-ref <native-followup-receipt>`, then re-enter bootstrap; this is the same-child live binding writeback and must not be replaced by spawn. If the host returns target/path/id not found, record `direnv exec . asp agent session observe-host-tree --name {name} --resident-target-status absent`, then re-enter bootstrap. Do not close a host-visible child, spawn a duplicate, or retry a historical child ID."
    )
}

pub(super) fn orphan_replacement_action(name: &str, managed_agent_kind: &str) -> String {
    if managed_agent_kind == "unknown-resident-agent-kind" {
        return format!(
            "Resident {name} has no configured managed agent kind. Do not replace the orphaned owner with a fallback child, generic agent, or normal thread; report host-agent-type-unavailable and keep only this resident lane blocked."
        );
    }

    format!(
        "The fresh native host-tree receipt proves /root/{managed_agent_kind} is absent. The registry owner is orphan-risk, so create exactly one child with agent_type={managed_agent_kind}, task_name={managed_agent_kind}, and fork_turns=none. SubagentStart atomically releases the orphaned registry owner and registers the new native identity."
    )
}

pub(super) fn typed_spawn_audit_action(name: &str) -> &'static str {
    if name == "asp-testing" {
        "Inspect collaboration.spawn_agent and record the result with `direnv exec . asp agent session observe-host-capability --name asp-testing --agent-type-field present|absent`, then re-enter bootstrap. The schema must expose agent_type=asp_testing; do not create anything before this receipt exists."
    } else {
        "Before Create, inspect the currently exposed native collaboration.spawn_agent tool schema. It must contain an agent_type field that can be set to asp_explorer. task_name, message, and fork_turns alone are not typed-spawn capability. Record the observation through `direnv exec . asp agent session observe-host-capability --name asp-explore --agent-type-field present|absent`; do not merely report it in prose. Then re-enter bootstrap. If agent_type is absent, do not create any child."
    }
}

pub(super) fn managed_resident_create_action(name: &str, managed_agent_kind: &str) -> String {
    if managed_agent_kind == "unknown-resident-agent-kind" {
        return format!(
            "Resident {name} has no configured managed agent kind. Do not create a fallback child, generic agent, or normal thread; report host-agent-type-unavailable and keep only this resident lane blocked."
        );
    }

    format!(
        "Only when rollout history and the native host agent tree both prove that no reusable ASP resident child exists, use a platform-native creation surface that explicitly exposes agent_type. Set agent_type={managed_agent_kind} to select the registered profile, task_name={managed_agent_kind} to reserve the canonical /root/{managed_agent_kind} path, and fork_turns=none, then create once. message text does not select the registered profile. If agent_type is not exposed, report host-agent-type-unavailable; do not create generic fallback agents or normal threads. Re-enter this pane after the native create call returns so SubagentStart can be audited."
    )
}

pub(super) fn runtime_observation_action(name: &str) -> &'static str {
    if name == "asp-testing" {
        "Use the main agent's native follow-up/resume surface for /root/asp_testing. Accept a fresh same-root current-runtime or agent-message identity observation; a resumed child is not required to emit a second SubagentStart. Re-enter bootstrap after the observation. Missing observation is not drift."
    } else {
        "Use the main agent's native follow-up/resume surface for the same canonical /root/asp_explorer target. Accept a fresh same-root current-runtime or agent-message identity observation; a resumed child is not required to emit a second SubagentStart. Re-enter bootstrap after the observation. Missing observation is not drift: do not retire the child or create a replacement."
    }
}

pub(super) fn ready_dispatch_action(_name: &str) -> &'static str {
    "Use the parser-owned derived-mode `asp agent session dispatch-claim` action for the exact denied command and configured receipt kind. Only action=send may execute once; action=wait/complete must poll the same semantic inputs and must never resend, reuse another receipt, or concatenate a second output block."
}

pub(super) fn model_repair_action(name: &str) -> &'static str {
    if name == "asp-testing" {
        "Retire the drifted /root/asp_testing child, wait for path release, then create one replacement with agent_type=asp_testing, task_name=asp_testing, and fork_turns=none. Re-enter bootstrap after its SubagentStart receipt."
    } else {
        "Use the host-native retire/archive action for the existing canonical child and wait for terminal status and /root/asp_explorer path release. Then create exactly one replacement through a spawn surface that exposes agent_type, with agent_type=asp_explorer, task_name=asp_explorer, and fork_turns=none. agent_type selects the registered role; task_name reserves its canonical collaboration path; message/natural-language task text is only payload. Let Codex load the full role configuration from ~/.codex/agents or the active higher-precedence agents directory; do not copy model or reasoning values into task text. If agent_type is unavailable, choose the blocker option instead of spawning. Re-enter this pane only after the replacement SubagentStart receipt."
    }
}
