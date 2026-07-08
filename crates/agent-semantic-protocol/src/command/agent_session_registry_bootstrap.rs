use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::agent_session_registry::{
    AgentSessionInteractiveMenu, AgentSessionLookupRequest, AgentSessionRegistry,
    ResidentChildBootstrapMenuInput, resident_child_bootstrap_menu,
};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_state::{project_session_scope_id, resolved_root_session_id};

pub(super) fn bootstrap_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    let project_id = project_session_scope_id(registry, project_root)?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let name = args.name.as_deref().unwrap_or("asp-explore");
    let mut record = if let Some(root_session_id) = root_session_id.as_deref() {
        registry.lookup_session(AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: args.child_session_id.as_deref(),
            root_session_id: Some(root_session_id),
            name: Some(name),
        })?
    } else {
        None
    };
    let now = unix_timestamp()?;
    let mut rollout_history_status = "not-needed";
    let mut rollout_history_action = "none";
    let registry_routable = record
        .as_ref()
        .map(|session| {
            !matches!(session.status.as_str(), "archived" | "closed") && session.is_routable_at(now)
        })
        .unwrap_or(false);
    if !registry_routable {
        let preflight =
            super::agent_session_registry_profile::adopt_reusable_rollout_session_before_create(
                registry,
                &project_id,
                root_session_id.as_deref(),
                args,
                Some(name),
                record.as_ref().map(|session| session.session_id.as_str()),
                now,
            )?;
        rollout_history_status = preflight.status;
        rollout_history_action = preflight.action;
        if let Some(adopted) = preflight.record {
            record = Some(adopted);
        }
    }
    let expected_model =
        super::agent_session_registry_validation::expected_model_for_session_profile(
            record
                .as_ref()
                .map_or(name, |session| session.name.as_str()),
            record.as_ref().map_or("", |session| session.role.as_str()),
        )?;
    let platform =
        crate::command::agent_session_registry::active_platform().unwrap_or("{platform}");
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform,
        name,
        root_session_id: root_session_id.as_deref(),
        record: record.as_ref(),
        expected_model: expected_model.as_deref(),
        rollout_history_status: Some(rollout_history_status),
        rollout_history_action: Some(rollout_history_action),
        now,
    });
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&menu)
                .map_err(|error| format!("failed to render bootstrap menu: {error}"))?
        );
    } else {
        print_bootstrap_menu(&menu);
    }
    Ok(())
}

fn print_bootstrap_menu(menu: &AgentSessionInteractiveMenu<'_>) {
    let state = format!("{:?}", menu.state);
    println!(
        "pane: asp.session.{}.v{}",
        state.to_ascii_lowercase(),
        menu.schema_version
    );
    println!("state: {state}");
    println!("target: {}", menu.name);
    println!("why: {}", latest_trace_result(menu));
    println!(
        "must: do not run denied ASP reasoning/search in the main agent; choose one number and use native transport"
    );
    println!(
        "transport: native.{}.{} using required outputs {}",
        menu.host_requirement.platform,
        menu.host_requirement.required_transport,
        comma_join(menu.host_requirement.required_outputs)
    );
    println!(
        "target-id: agentMessageTargetId is the host-native message address; use a distinct target id when the host returns one, or the host single agent id only after native message-agent send accepts it; never derive it from a normal thread id or rollout path"
    );
    if let Some(expected_model) = menu.expected_model {
        println!("model: expected {expected_model}");
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
    } else {
        println!(
            "session: none; choose Create only after rollout audit found no reusable ASP-managed resident child"
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
        "select: return exactly one number, such as \"1\"; perform its do action through native transport; return expect; re-enter the loop at after."
    );
}

fn latest_trace_result(menu: &AgentSessionInteractiveMenu<'_>) -> String {
    menu.trace
        .last()
        .map(|step| step.result)
        .or(menu.rollout_history_status)
        .unwrap_or("loop state requires action")
        .to_string()
}

fn comma_join(values: &[&str]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(",")
    }
}

fn required_inputs_phrase(values: &[&str]) -> String {
    if values.is_empty() {
        "native action observation".to_string()
    } else {
        values.join(",")
    }
}

fn unix_timestamp() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|error| format!("system clock before unix epoch: {error}"))
}
