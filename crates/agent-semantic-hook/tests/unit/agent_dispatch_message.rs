// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::render_collaboration_instruction;

#[test]
fn collaboration_message_names_the_configured_agent_and_native_tool() {
    let message = render_collaboration_instruction(
        Some("asp_explorer"),
        Some("parent-thread-1"),
        "Run `asp search playbook --language rust --rg HookDecision` exactly once",
    );

    assert!(message.contains("collaboration.spawn_agent"));
    assert!(message.contains("collaboration.list_agents"));
    assert!(message.contains("agent_type: \"asp_explorer\""));
    assert!(message.contains("task_name: \"asp_explorer\""));
    assert!(message.contains(
        "asp session register-child --parent-thread-id parent-thread-1 --agent-name asp_explorer"
    ));
    assert!(message.contains("reads the shared root session id and current child thread id"));
    assert!(message.contains("Host-native authority"));
    assert!(message.contains("sandbox_permissions: \"require_escalated\""));
    assert!(message.contains("Do not first launch it inside the child sandbox"));
    assert!(
        message
            .contains("Run `asp search playbook --language rust --rg HookDecision` exactly once")
    );
    assert!(message.contains("execute this parent-authored task exactly as written"));
    assert!(message.contains("/root/asp_explorer"));
    assert!(message.contains("attached typed"));
    assert!(message.contains("multi_agent_v2"));
    assert!(message.contains("codex-multi-agent-v2-capability-unavailable"));
    assert!(!message.contains("multi_agent_v1"));
    assert!(message.contains("host-agent-observation-capability-unavailable"));
    assert!(message.contains("Never emit or execute an unavailable tool spelling"));
    assert!(
        message.find("requiredAction=followup-task")
            < message.find("requiredAction=observe-host-path"),
        "follow-up must precede Host-tree observation"
    );
    assert!(!message.contains("First inspect the Codex Host"));
    assert!(message.contains("collaboration.list_agents({\n  path_prefix: \"/root\"\n})"));
    assert!(message.contains("collaboration.followup_task({"));
    assert!(message.contains("collaboration.send_message({"));
    assert!(
        message.contains("collaboration.interrupt_agent({\n  target: \"/root/asp_explorer\"\n})")
    );
    assert!(message.contains("collaboration.wait_agent({\n  timeout_ms: 10000\n})"));
    assert!(message.contains("Resume the existing configured Agent at `/root/asp_explorer`"));
    assert!(
        message.contains(
            "Do not create another Agent and do not run `asp session register-child` again"
        )
    );
    assert!(
        message.contains("Continue the already running configured Agent at `/root/asp_explorer`")
    );
    assert!(message.contains("It never starts a turn, so it is not a dispatch path"));
    assert!(
        message
            .contains("whether the observed status is running, interrupted, completed, or errored")
    );
    assert!(message.contains("prevents a Host snapshot race"));
    assert!(message.contains("receipt is missing or stale"));
    assert!(message.contains("standardized JSON"));
    assert!(message.contains("Host path/status"));
    assert!(message.contains("does not replace the Registry disposition"));
    assert!(message.contains("existing AgentSession Registry"));
    assert!(message.contains("does not create a second filesystem mirror"));
    assert!(message.contains("`asp clean --day` owns expiry"));
    assert!(message.contains("requiredAction=followup-task"));
    assert!(message.contains("interrupt that same canonical Agent path without deleting"));
    assert!(message.contains("Waiting only observes activity"));
    assert!(
        message.contains("never\ncreates, resumes, registers, interrupts, or authorizes an Agent")
    );
    assert!(message.contains("A DB row never invents an Agent or grants a permission"));
    assert!(message.contains("Do not invent either session id or an Agent path"));
    assert!(!message.contains("{{{"));
    assert!(!message.contains("}}}"));
    assert!(!message.contains("choice-plane"));
    assert!(!message.contains("self-registration"));
    assert!(message.contains("message: \"First run `asp session"));
    assert!(message.contains("sandbox_permissions: \\\"require_escalated\\\""));
    assert!(!message.contains("sandbox_permissions: \"require_escalated\" on that first"));
}
