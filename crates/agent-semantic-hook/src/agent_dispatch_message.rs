use std::sync::LazyLock;

use orgize::{Org, ast::NamedSourceBlockTemplate};

const COLLABORATION_ORG_CONTRACT: &str =
    include_str!("../../../org/contracts/agent.multi-agent-session-control-plane.v1.org");

struct CollaborationTemplates {
    list_agents: NamedSourceBlockTemplate,
    spawn_agent: NamedSourceBlockTemplate,
    followup_task: NamedSourceBlockTemplate,
    send_message: NamedSourceBlockTemplate,
    interrupt_agent: NamedSourceBlockTemplate,
    wait_agent: NamedSourceBlockTemplate,
    registration_message: NamedSourceBlockTemplate,
    resume_message: NamedSourceBlockTemplate,
    continue_message: NamedSourceBlockTemplate,
    dispatch_message: NamedSourceBlockTemplate,
}

static COLLABORATION_TEMPLATES: LazyLock<CollaborationTemplates> = LazyLock::new(|| {
    let contract = Org::parse(COLLABORATION_ORG_CONTRACT).document();
    let block = |name, language| {
        contract
            .named_source_block_template(name, language)
            .unwrap_or_else(|error| panic!("invalid Org collaboration block `{name}`: {error}"))
    };
    CollaborationTemplates {
        list_agents: block("collaboration-list-agents", "codex-collaboration"),
        spawn_agent: block("collaboration-spawn-agent", "codex-collaboration"),
        followup_task: block("collaboration-followup-task", "codex-collaboration"),
        send_message: block("collaboration-send-message", "codex-collaboration"),
        interrupt_agent: block("collaboration-interrupt-agent", "codex-collaboration"),
        wait_agent: block("collaboration-wait-agent", "codex-collaboration"),
        registration_message: block("collaboration-registration-message", "text"),
        resume_message: block("collaboration-resume-message", "text"),
        continue_message: block("collaboration-continue-message", "text"),
        dispatch_message: block("collaboration-dispatch-message", "text"),
    }
});

pub(crate) fn render_collaboration_instruction(
    target_agent: Option<&str>,
    parent_thread_id: Option<&str>,
    parent_task: &str,
) -> String {
    let agent_type = target_agent
        .filter(|agent| !agent.is_empty())
        .unwrap_or("configured_agent");
    let agent_path = format!("/root/{agent_type}");
    let parent_thread_id = parent_thread_id
        .filter(|thread_id| !thread_id.is_empty())
        .unwrap_or("<parent-thread-id>");
    let registration_command = format!(
        "asp session register-child --parent-thread-id {parent_thread_id} --agent-name {agent_type}"
    );
    assert!(
        !parent_task.trim().is_empty(),
        "Collaboration parent task must be concrete"
    );
    let templates = &*COLLABORATION_TEMPLATES;
    let registration_message = render_message(
        &templates.registration_message,
        &agent_path,
        &registration_command,
        parent_task,
    );
    let resume_message = render_message(
        &templates.resume_message,
        &agent_path,
        &registration_command,
        parent_task,
    );
    let continue_message = render_message(
        &templates.continue_message,
        &agent_path,
        &registration_command,
        parent_task,
    );
    let list_agents = templates
        .list_agents
        .render([])
        .expect("render collaboration.list_agents contract");
    let spawn = render_call(
        &templates.spawn_agent,
        agent_type,
        &agent_path,
        &registration_message,
    );
    let followup = render_call(
        &templates.followup_task,
        agent_type,
        &agent_path,
        &resume_message,
    );
    let followup_registration = render_call(
        &templates.followup_task,
        agent_type,
        &agent_path,
        &registration_message,
    );
    let send = render_call(
        &templates.send_message,
        agent_type,
        &agent_path,
        &continue_message,
    );
    let interrupt = render_call(
        &templates.interrupt_agent,
        agent_type,
        &agent_path,
        &continue_message,
    );
    let wait = templates
        .wait_agent
        .render([])
        .expect("render collaboration.wait_agent contract");
    templates
        .dispatch_message
        .render([
            ("AGENT_PATH", agent_path.as_str()),
            ("LIST_AGENTS_CALL", list_agents.as_str()),
            ("SPAWN_AGENT_CALL", spawn.as_str()),
            ("FOLLOWUP_TASK_CALL", followup.as_str()),
            ("FOLLOWUP_REGISTRATION_CALL", followup_registration.as_str()),
            ("SEND_MESSAGE_CALL", send.as_str()),
            ("INTERRUPT_AGENT_CALL", interrupt.as_str()),
            ("WAIT_AGENT_CALL", wait.as_str()),
            ("REGISTRATION_COMMAND", registration_command.as_str()),
        ])
        .unwrap_or_else(|error| panic!("invalid Org collaboration dispatch contract: {error}"))
}

fn render_message(
    template: &NamedSourceBlockTemplate,
    agent_path: &str,
    registration_command: &str,
    parent_task: &str,
) -> String {
    template
        .render(
            [
                ("AGENT_PATH", agent_path),
                ("REGISTRATION_COMMAND", registration_command),
                ("PARENT_TASK", parent_task),
            ]
            .into_iter()
            .filter(|(binding, _)| match template.name() {
                "collaboration-registration-message" => {
                    matches!(*binding, "REGISTRATION_COMMAND" | "PARENT_TASK")
                }
                _ => matches!(*binding, "AGENT_PATH" | "PARENT_TASK"),
            }),
        )
        .unwrap_or_else(|error| {
            panic!(
                "invalid Org collaboration message `{}`: {error}",
                template.name()
            )
        })
}

fn render_call(
    template: &NamedSourceBlockTemplate,
    agent_type: &str,
    agent_path: &str,
    message: &str,
) -> String {
    let message_json = serde_json::to_string(message).expect("encode Collaboration message");
    template
        .render(
            [
                ("AGENT_TYPE", agent_type),
                ("AGENT_PATH", agent_path),
                ("MESSAGE", message_json.as_str()),
            ]
            .into_iter()
            .filter(|(binding, _)| match template.name() {
                "collaboration-spawn-agent" => *binding != "AGENT_PATH",
                "collaboration-interrupt-agent" => *binding == "AGENT_PATH",
                _ => *binding != "AGENT_TYPE",
            }),
        )
        .unwrap_or_else(|error| {
            panic!(
                "invalid Org collaboration block `{}`: {error}",
                template.name()
            )
        })
}

#[cfg(test)]
#[path = "../tests/unit/agent_dispatch_message.rs"]
mod tests;
