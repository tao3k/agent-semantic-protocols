// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! PreTool structural and native admission for the public Search Playbook command.

use agent_semantic_shell_parser::CommandStage;
use serde_json::{Value, json};

pub fn evaluate(payload_json: &str, host_matcher: &str) -> Result<Option<Value>, String> {
    if host_matcher != "Bash" {
        return Ok(None);
    }
    let payload: Value = serde_json::from_str(payload_json)
        .map_err(|error| format!("decode Hook payload for Search calibration: {error}"))?;
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return Ok(None);
    }
    let Some(command) = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .and_then(|input| input.get("command").or_else(|| input.get("cmd")))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    let stages = agent_semantic_shell_parser::parse_bash_command_candidates(command)
        .map_err(|error| format!("parse Bash command for Search calibration: {error}"))?;
    for stage in stages.iter().filter(|stage| !stage.is_separator()) {
        let Some(args) = search_playbook_args(stage) else {
            continue;
        };
        let Err(error) = agent_semantic_search::parse_progressive_search_playbook_args(args) else {
            continue;
        };
        let source = args.get(2).map(String::as_str);
        let mut issue = json!({
            "reasonKind": error.reason_kind(),
            "field": "expression",
            "message": error.to_string(),
        });
        if source.is_some() {
            issue["tokenIndex"] = json!(2);
        }
        let calibration = json!({
            "schemaId": "agent.semantic-protocols.search-playbook-pretool-calibration",
            "schemaVersion": "1",
            "state": "rejected",
            "reasonKind": "search-playbook-pretool-calibration",
            "tokenIndexBasis": "search-playbook-argv",
            "layout": {
                "layoutId": "rg-tantivy-structural-scope",
                "shape": "intersect(rg,tantivy)->structural-scope-facts->graph?",
                "requiredInputSets": [["rg", "tantivy"]],
                "graphBarrierAfter": "structural-scope-facts",
            },
            "producers": [],
            "inputOccurrences": [],
            "rgArgvBoundaries": [],
            "rgAnalyses": [],
            "tantivyAnalyses": [],
            "missingFields": if source.is_none() { vec!["expression"] } else { vec![] },
            "conflictingFields": [],
            "issues": [issue],
        });
        let encoded = serde_json::to_string(&calibration)
            .map_err(|error| format!("encode Search calibration: {error}"))?;
        let summary = "ASP Search Playbook was rejected before execution; additionalContext contains the typed Scheme-source calibration.";
        return Ok(Some(json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": summary,
                "additionalContext": encoded,
            },
            "systemMessage": summary,
        })));
    }
    Ok(None)
}

fn search_playbook_args(stage: &CommandStage) -> Option<&[String]> {
    let words = stage.words();
    words.iter().enumerate().find_map(|(asp_index, word)| {
        (std::path::Path::new(word)
            .file_name()
            .is_some_and(|name| name == "asp")
            && words.get(asp_index + 1).map(String::as_str) == Some("search")
            && words.get(asp_index + 2).map(String::as_str) == Some("playbook"))
        .then_some(&words[asp_index + 1..])
    })
}
