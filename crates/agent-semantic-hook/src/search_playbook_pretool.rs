// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! PreTool fill-form calibration for the public Search Playbook command.

use agent_semantic_shell_parser::{
    CommandStage, SearchPlaybookBlockKind, SearchPlaybookGlobalKind,
    parse_search_playbook_boundaries,
};
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
        let parsed = parse_search_playbook_boundaries(args);
        if parsed.is_valid() {
            continue;
        }
        let producers = parsed
            .globals
            .iter()
            .filter(|global| global.kind == SearchPlaybookGlobalKind::Language)
            .map(|global| {
                json!({
                    "field": global.kind.field_name(),
                    "value": global.value,
                    "optionTokenIndex": global.option_token_index,
                    "valueTokenIndex": global.value_token_index,
                })
            })
            .collect::<Vec<_>>();
        let input_occurrences = parsed
            .blocks
            .iter()
            .map(input_occurrence)
            .collect::<Vec<_>>();
        let rg_argv_boundaries = parsed
            .blocks
            .iter()
            .filter(|block| block.kind == SearchPlaybookBlockKind::Rg)
            .map(input_occurrence)
            .collect::<Vec<_>>();
        let rg_analyses = parsed
            .blocks
            .iter()
            .filter(|block| block.kind == SearchPlaybookBlockKind::Rg)
            .zip(parsed.rg_analyses.iter())
            .map(|(block, analysis)| {
                json!({
                    "blockIndex": block.block_index,
                    "admitted": analysis.is_admitted(),
                    "exactArgv": analysis.argv,
                    "outputAttribution": analysis.output_attribution.as_str(),
                    "options": analysis.options.iter().map(|option| {
                        let mut value = json!({
                            "option": option.option,
                            "optionTokenIndex": option.option_token_index,
                        });
                        if let Some(value_token_index) = option.value_token_index {
                            value["valueTokenIndex"] = json!(value_token_index);
                        }
                        if let Some(option_value) = option.value.as_ref() {
                            value["value"] = json!(option_value);
                        }
                        value
                    }).collect::<Vec<_>>(),
                    "patterns": analysis.patterns.iter().map(|pattern| {
                        let mut value = json!({"value": pattern.value});
                        if let Some(token_index) = pattern.token_index {
                            value["tokenIndex"] = json!(token_index);
                        }
                        value
                    }).collect::<Vec<_>>(),
                    "searchRoots": analysis.search_roots.iter().map(|root| {
                        let mut value = json!({"value": root.value});
                        if let Some(token_index) = root.token_index {
                            value["tokenIndex"] = json!(token_index);
                        }
                        value
                    }).collect::<Vec<_>>(),
                    "diagnostics": analysis.diagnostics.iter().map(|diagnostic| {
                        let mut value = json!({
                            "reasonKind": diagnostic.kind.reason_kind(),
                            "message": diagnostic.message,
                        });
                        if let Some(token_index) = diagnostic.argv_token_index {
                            value["argvTokenIndex"] = json!(token_index);
                        }
                        value
                    }).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        let issues = parsed
            .issues
            .iter()
            .map(|issue| {
                let mut value = json!({
                    "reasonKind": issue.kind.reason_kind(),
                    "field": issue.field,
                    "message": issue.message,
                });
                if let Some(token_index) = issue.token_index {
                    value["tokenIndex"] = json!(token_index);
                }
                value
            })
            .collect::<Vec<_>>();
        let tantivy_analyses = parsed
            .blocks
            .iter()
            .filter(|block| block.kind == SearchPlaybookBlockKind::Tantivy)
            .zip(parsed.tantivy_analyses.iter())
            .map(|(block, analysis)| {
                json!({
                    "blockIndex": block.block_index,
                    "expression": analysis.expression,
                    "admitted": analysis.is_admitted(),
                    "fields": analysis.fields,
                    "metrics": {
                        "leafCount": analysis.metrics.leaf_count,
                        "fieldedLeafCount": analysis.metrics.fielded_leaf_count,
                        "explicitBooleanCount": analysis.metrics.explicit_boolean_count,
                        "phraseCount": analysis.metrics.phrase_count,
                        "boostCount": analysis.metrics.boost_count,
                        "rangeCount": analysis.metrics.range_count,
                        "setCount": analysis.metrics.set_count,
                        "existsCount": analysis.metrics.exists_count,
                        "regexCount": analysis.metrics.regex_count,
                        "maxDepth": analysis.metrics.max_depth,
                    },
                    "syntaxDiagnostics": analysis.syntax_diagnostics.iter().map(|diagnostic| {
                        let mut value = json!({"message": diagnostic.message});
                        if let Some(char_offset) = diagnostic.char_offset {
                            value["charOffset"] = json!(char_offset);
                        }
                        value
                    }).collect::<Vec<_>>(),
                    "unsupportedFields": analysis.unsupported_fields,
                    "missingFeatures": analysis.missing_features,
                })
            })
            .collect::<Vec<_>>();
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
            "producers": producers,
            "inputOccurrences": input_occurrences,
            "rgArgvBoundaries": rg_argv_boundaries,
            "rgAnalyses": rg_analyses,
            "tantivyAnalyses": tantivy_analyses,
            "missingFields": parsed.missing_fields,
            "conflictingFields": parsed.conflicting_fields,
            "issues": issues,
        });
        let encoded = serde_json::to_string(&calibration)
            .map_err(|error| format!("encode Search calibration: {error}"))?;
        let summary = "ASP Search Playbook was rejected before execution; additionalContext contains the complete typed fill-form calibration.";
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

fn input_occurrence(block: &agent_semantic_shell_parser::SearchPlaybookNativeBlock) -> Value {
    json!({
        "field": block.kind.field_name(),
        "blockIndex": block.block_index,
        "optionTokenIndex": block.option_token_index,
        "argvStartTokenIndex": block.argv_start_token_index,
        "argvEndTokenIndexExclusive": block.argv_end_token_index_exclusive,
        "argv": block.argv,
    })
}
