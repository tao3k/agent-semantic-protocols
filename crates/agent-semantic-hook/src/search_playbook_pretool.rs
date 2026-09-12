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
        let grammar = source
            .and_then(|source| agent_semantic_search::admit_search_playbook_source(source).ok())
            .map(|receipt| {
                json!({
                    "grammarId": receipt.grammar_id,
                    "grammarVersion": receipt.grammar_version,
                    "grammarRepository": receipt.grammar_repository,
                    "byteLength": receipt.byte_len,
                    "rootKind": receipt.root_kind,
                    "topLevelFormCount": receipt.top_level_form_count,
                    "topLevelFormHeads": receipt.top_level_form_heads,
                })
            });
        let mut issue = json!({
            "reasonKind": error.reason_kind(),
            "field": "expression",
            "message": error.to_string(),
        });
        if source.is_some() {
            issue["sourceTokenIndex"] = json!(2);
        }
        let calibration = json!({
            "schemaId": "agent.semantic-protocols.search-playbook-pretool-calibration",
            "schemaVersion": "2",
            "state": "rejected",
            "reasonKind": "search-playbook-pretool-calibration",
            "argumentModel": "one-scheme-expression",
            "sourceDigest": source.map(source_digest),
            "expressionArgCount": args.len().saturating_sub(2),
            "grammar": grammar,
            "layout": {
                "layoutId": "rg-tantivy-structural-scope",
                "shape": "chain(intersect(rg,tantivy),syntax*,native-syntax*,graph*)",
                "graphBarrierAfter": "structural-scope-facts",
            },
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

fn source_digest(source: &str) -> String {
    format!("blake3-256:{}", blake3::hash(source.as_bytes()).to_hex())
}
