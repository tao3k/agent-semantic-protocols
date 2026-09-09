// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_hook::DecisionKind;
use agent_semantic_hook::DecisionRoute;
use agent_semantic_hook::DecisionRouteKind;
use agent_semantic_hook::DecisionSubject;
use agent_semantic_hook::HOOK_DECISION_SCHEMA_ID;
use agent_semantic_hook::HOOK_DECISION_SCHEMA_VERSION;
use agent_semantic_hook::HOOK_PROTOCOL_ID;
use agent_semantic_hook::HOOK_PROTOCOL_VERSION;
use agent_semantic_hook::HookDecision;
use agent_semantic_hook::ReasonKind;
use agent_semantic_hook::materialize_source_access_deny_message;

#[test]
fn configured_message_binds_language_and_appends_executable_provider_route() {
    let mut decision = HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: "codex".to_owned(),
        event: "pre-tool".to_owned(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::StructuredSourceRead,
        language_ids: vec![agent_semantic_config::LanguageId::new("rust")],
        subject: DecisionSubject::default(),
        routes: vec![DecisionRoute {
            language_id: agent_semantic_config::LanguageId::new("rust"),
            provider_id: agent_semantic_config::ProviderId::new("asp-rust"),
            binary: "asp".to_owned(),
            kind: DecisionRouteKind::Playbook,
            argv: vec![
                "asp".to_owned(),
                "search".to_owned(),
                "playbook".to_owned(),
                "--language".to_owned(),
                "rust".to_owned(),
                "--rg".to_owned(),
                "--files".to_owned(),
                "-g".to_owned(),
                "src/a file.rs".to_owned(),
                "--tantivy".to_owned(),
                "title:[* TO *] OR body:[* TO *]".to_owned(),
            ],
            stdin_mode: None,
        }],
        message: "Registered {{languageId}} source reads are denied.".to_owned(),
        fields: Default::default(),
    };

    materialize_source_access_deny_message(&mut decision);

    assert!(
        decision
            .message
            .starts_with("Registered rust source reads are denied.")
    );
    assert!(decision.message.contains(
        "ASP route: `asp search playbook --language rust --rg --files -g 'src/a file.rs' --tantivy 'title:[* TO *] OR body:[* TO *]'`"
    ));
}

#[test]
fn materialized_decision_shards_keep_denials_guided_and_role_only() {
    let shards = agent_semantic_hook::ClientHookConfig::default()
        .materialized_decision_shards()
        .expect("materialize default Hook decision shards");

    let mut table_count = 0usize;
    for (_extension, table) in shards.shell_read {
        agent_semantic_hook::CommandDecisionShard::map_binary_decisions(&table, |decision| {
            table_count += 1;
            assert_guided_role_only_deny(&decision);
            decision
        })
        .expect("decode materialized shell-read decisions");
    }
    agent_semantic_hook::CommandDecisionShard::map_binary_decisions(
        &shards.shell_command,
        |decision| {
            table_count += 1;
            assert_guided_role_only_deny(&decision);
            decision
        },
    )
    .expect("decode materialized shell-command decisions");
    assert!(
        table_count > 0,
        "default config must materialize table decisions"
    );
}

fn assert_guided_role_only_deny(decision: &HookDecision) {
    if decision.decision != DecisionKind::Deny {
        return;
    }
    assert!(!decision.message.trim().is_empty());
    assert!(!decision.message.contains("targetAgentName"));
    assert!(!decision.fields.contains_key("targetAgentName"));
    assert!(!decision.fields.contains_key("residentChildName"));
}
