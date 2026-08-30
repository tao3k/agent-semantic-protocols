use std::sync::LazyLock;

use orgize::{Org, ast::NamedSourceBlockTemplate};

const COLLABORATION_ORG_CONTRACT: &str =
    include_str!("../../../org/contracts/agent.multi-agent-session-control-plane.v1.org");

struct CollaborationTemplates {
    list_agents: NamedSourceBlockTemplate,
    spawn_agent: NamedSourceBlockTemplate,
    followup_task: NamedSourceBlockTemplate,
    send_message: NamedSourceBlockTemplate,
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
        dispatch_message: block("collaboration-dispatch-message", "text"),
    }
});

pub(crate) fn render_collaboration_instruction(target_agent: Option<&str>) -> String {
    let agent_type = target_agent
        .filter(|agent| !agent.is_empty())
        .unwrap_or("configured_agent");
    let agent_path = format!("/root/{agent_type}");
    let message = "Perform the denied operation and return a compact receipt.";
    let templates = &*COLLABORATION_TEMPLATES;
    let list_agents = templates
        .list_agents
        .render([])
        .expect("render collaboration.list_agents contract");
    let spawn = render_call(&templates.spawn_agent, agent_type, &agent_path, message);
    let followup = render_call(&templates.followup_task, agent_type, &agent_path, message);
    let send = render_call(&templates.send_message, agent_type, &agent_path, message);
    templates
        .dispatch_message
        .render([
            ("AGENT_PATH", agent_path.as_str()),
            ("LIST_AGENTS_CALL", list_agents.as_str()),
            ("SPAWN_AGENT_CALL", spawn.as_str()),
            ("FOLLOWUP_TASK_CALL", followup.as_str()),
            ("SEND_MESSAGE_CALL", send.as_str()),
        ])
        .unwrap_or_else(|error| panic!("invalid Org collaboration dispatch contract: {error}"))
}

fn render_call(
    template: &NamedSourceBlockTemplate,
    agent_type: &str,
    agent_path: &str,
    message: &str,
) -> String {
    template
        .render(
            [
                ("AGENT_TYPE", agent_type),
                ("AGENT_PATH", agent_path),
                ("MESSAGE", message),
            ]
            .into_iter()
            .filter(|(binding, _)| match template.name() {
                "collaboration-spawn-agent" => *binding != "AGENT_PATH",
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
