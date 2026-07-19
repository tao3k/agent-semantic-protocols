use agent_semantic_client_db::agent_session_registry::AgentSessionInteractiveMenu;

use super::{
    canonical_resident_target, codex_spawn_agent_metadata_capability, latest_trace_result,
    main_agent_runtime_rebind_instruction, platform_native_create_action, runtime_repair_diagnosis,
};

pub(super) fn print_bootstrap_menu(
    menu: &AgentSessionInteractiveMenu<'_>,
    runtime_drift: Option<&agent_semantic_hook::SubagentRuntimeDriftObservation>,
    runtime_verified: Option<&agent_semantic_hook::SubagentRuntimeRebindVerifiedObservation>,
) {
    let state = format!("{:?}", menu.state);
    println!(
        "pane: asp.session.{}.v{}",
        state.to_ascii_lowercase(),
        menu.schema_version
    );
    println!("state: {state}");
    println!("target: {}", menu.name);
    if let Some(root_session_id) = menu.root_session_id {
        println!("root-session: {root_session_id}");
    }
    let why = runtime_verified.map_or_else(
        || {
            runtime_drift.map_or_else(
                || latest_trace_result(menu),
                |_| "typed-resident-replacement-required".to_string(),
            )
        },
        |_| "typed-resident-replacement-verified".to_string(),
    );
    println!("why: {why}");
    if let Some(observation) = runtime_verified {
        println!(
            "typed-replacement-verification: verified=true child={} sameChildIdentity=false source={} observationCount={} previousModel={} observedModel={} expectedModel={} previousReasoning={} observedReasoning={} expectedReasoning={} modelMatchesExpected=true reasoningVerification=observation-or-profile-attestation registryRoutable={}",
            observation.child_session_id,
            observation.observation_source,
            observation.observation_count,
            observation
                .previous_observed_model
                .as_deref()
                .unwrap_or("unknown"),
            observation.observed_model,
            observation.expected_model,
            observation
                .previous_observed_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            observation
                .observed_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            observation
                .expected_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            menu.session.is_some(),
        );
    }
    if let Some(observation) = runtime_drift {
        let diagnosis = runtime_repair_diagnosis(menu, observation);
        println!(
            "host-lifecycle: child={} expectedAgentType={} observedAgentType={} expectedModel={} observedModel={} expectedReasoning={} observedReasoning={} driftDimensions={} consecutiveObservationCount={} repairAttemptStatus=typed-resident-replacement-required action=retire-drifted-child-and-create-configured-replacement runtimeOverrideOwner=none runtimeSwitchIntentInFollowupMessage=false preserveChildIdentity=false managedProfileAttestation={}",
            observation.child_session_id,
            observation.expected_agent_type,
            observation.observed_agent_type,
            menu.expected_model.unwrap_or("unknown"),
            observation.observed_model.as_deref().unwrap_or("unknown"),
            menu.expected_reasoning_effort.unwrap_or("unknown"),
            observation
                .observed_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            diagnosis.drift_dimensions.join(","),
            observation.consecutive_observation_count,
            if observation.observed_agent_type == observation.expected_agent_type {
                "observed-agent-type-match"
            } else {
                "not-established-by-host-agent-type"
            },
        );
        println!(
            "repair: retire/archive the drifted child, wait for terminal host status and canonical-path release, then create exactly one replacement with agent_type={}, task_name={}, and fork_turns=none. Codex must load the registered TOML; do not send a natural-language model switch.",
            menu.host_requirement.managed_agent_kind.as_ref(),
            menu.host_requirement.managed_agent_kind.as_ref(),
        );
        let canonical_target = canonical_resident_target(&menu.host_requirement);
        println!(
            "main-agent-control-directive: {}",
            main_agent_runtime_rebind_instruction(
                &canonical_target,
                observation,
                menu.host_requirement.managed_agent_kind.as_ref(),
            )
        );
        println!(
            "host-control-contract: target={} identityPolicy=retire-before-replacement createPolicy=single-typed-replacement-only instructionMode=host-native-lifecycle requiredSurface=host-native-retire-and-typed-spawn requiredParameters=target,agent_type,task_name,fork_turns runtimeApplication=codex-registered-role-config taskMessageCarriesControlIntent=false verification=typed-SubagentStart",
            canonical_target
        );
        println!(
            "host-control-blocker: nextState=Blocked bootstrapBlocked={} driftDimensions={} policy=ASP-routing-degraded-but-unrelated-Codex-tools-allowed",
            diagnosis.bootstrap_blocked,
            diagnosis.drift_dimensions.join(","),
        );
    }
    println!(
        "must: require a profile-valid resident child with verified live binding; missing host capability blocks only resident-routed commands and never authorizes inline, generic-child, historical-target, or raw-source substitution"
    );
    println!(
        "transport: required=native.{}.{}; resident lifecycle identity is captured by host hooks",
        menu.host_requirement.platform, menu.host_requirement.required_transport
    );
    if menu.host_requirement.platform == "codex" {
        println!(
            "profile-discovery: Codex auto-loads custom-agent TOML from ~/.codex/agents/ and .codex/agents/; bootstrap does not write [agents.<name>] registration"
        );
        let metadata = codex_spawn_agent_metadata_capability();
        println!(
            "host-typed-spawn-projection: spawnAgentMetadata={} agentTypeProjected={} metadataHiddenByConfig={} diagnosticOnly=true lifecycleAuthority=live-host-spawn-schema-and-agent-tree-audit missingCapabilityPolicy=local-resident-command-blocked",
            metadata,
            metadata == "visible-agent-type",
            metadata == "hidden-by-config",
        );
    }
    println!(
        "resident-registration: SubagentStart event owns registration and validation of the spawned runtime child identity"
    );
    let has_create_choice = menu
        .choices
        .iter()
        .any(|choice| choice.id == "create-managed-resident-child-after-host-tree-miss");
    if has_create_choice {
        println!(
            "host-typed-spawn-preflight: tool=collaboration.spawn_agent requiredField=agent_type requiredValue={} genericFieldsInsufficient=task_name,message,fork_turns unavailable=bootstrapBlocked=host-agent-type-unavailable",
            menu.host_requirement.managed_agent_kind.as_ref(),
        );
        println!(
            "platform-native-create: platform={} managedAgentKind={} action={}",
            menu.host_requirement.platform,
            menu.host_requirement.managed_agent_kind.as_ref(),
            platform_native_create_action(&menu.host_requirement)
        );
        println!(
            "platform-native-create-blocker: Create is authorized only after rollout and host-agent-tree audits both miss; if platform={} cannot audit or create managedAgentKind={} or emits no SubagentStart event, report the matching host lifecycle gap; do not create fallback agents or normal threads",
            menu.host_requirement.platform, menu.host_requirement.managed_agent_kind
        );
    }
    if let Some(expected_model) = menu.expected_model {
        println!("model: expected {expected_model}");
    }
    if let Some(expected_reasoning_effort) = menu.expected_reasoning_effort {
        println!("reasoning: expected {expected_reasoning_effort}");
    }
    if let Some(status) = menu.rollout_history_status {
        println!(
            "rollout: status={} action={}",
            status,
            menu.rollout_history_action.unwrap_or("none")
        );
    }
    if let Some(session) = menu.session.as_ref() {
        println!(
            "session: child={} status={} role={} model={} messageTarget={}",
            session.child_session_id,
            session.status,
            session.role,
            session.model.unwrap_or("unknown"),
            session.message_target_status
        );
        if let Some(source) = session.model_observation_source {
            println!(
                "model-observation: source={} observedAt={} evidence={}",
                source,
                session.model_observed_at.unwrap_or_default(),
                session.model_evidence_ref.unwrap_or("none")
            );
        }
    } else {
        println!(
            "registry: missing; this does not mean the resident child is absent; audit rollout history and the native host agent tree before Create"
        );
    }
    println!();
    for (index, choice) in menu.choices.iter().enumerate() {
        println!("{}: {}", index + 1, choice.id);
        println!("  ask: {}", choice.label);
        println!("  do: {}", choice.platform_action);
        println!(
            "  expect: {}",
            required_inputs_phrase(choice.required_inputs)
        );
        println!("  after: {:?}", choice.next_state);
        println!();
    }
    println!(
        "select: return exactly one number, such as \"1\"; perform its do action through the declared transport; return expect; re-enter the loop at after."
    );
    println!(
        "lifecycle: after native create, typed replacement, or interrupt, re-enter this pane; never pass child ids, message targets, model claims, or lifecycle status as bootstrap command flags."
    );
}

fn required_inputs_phrase(required_inputs: usize) -> String {
    match required_inputs {
        0 => "no required inputs".to_string(),
        1 => "1 required input".to_string(),
        count => format!("{count} required inputs"),
    }
}
