use crate::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, HookRuntime, ReasonKind,
    classify_hook_with_config,
};
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::BTreeSet;

thread_local! {
    static SYNTHETIC_MATCH_ENVIRONMENT: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn synthetic_match_environment_active() -> bool {
    SYNTHETIC_MATCH_ENVIRONMENT.get()
}

fn with_synthetic_match_environment<T>(evaluate: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            SYNTHETIC_MATCH_ENVIRONMENT.set(self.0);
        }
    }
    let previous = SYNTHETIC_MATCH_ENVIRONMENT.replace(true);
    let _reset = Reset(previous);
    evaluate()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPolicyConformanceReport {
    pub configured_rule_count: usize,
    pub case_count: usize,
    pub covered_rule_ids: BTreeSet<String>,
    pub failures: Vec<String>,
}

impl MatchPolicyConformanceReport {
    pub fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.covered_rule_ids.len() == self.configured_rule_count
    }
}

struct MatchPolicyCase {
    name: &'static str,
    payload: Value,
    rule_id: &'static str,
    decision: DecisionKind,
    reason: ReasonKind,
}

fn shell(command: &str) -> Value {
    json!({"tool_name": "exec_command", "tool_input": {"cmd": command}})
}

/// Execute the canonical production witness for every managed Hook rule.
///
/// The configured rule-id set must equal the witness set. Custom or drifted
/// configurations therefore fail closed instead of inheriting a misleading
/// `complete` status from the embedded defaults.
pub fn evaluate_match_policy_conformance(
    runtime: &HookRuntime,
    config: &ClientHookConfig,
    platform: &str,
) -> MatchPolicyConformanceReport {
    let agent_search_binary = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id.as_str() == "typescript")
        .map(|_| "asp");
    let cases = production_cases(agent_search_binary);
    let config = match config.match_policy_conformance_config() {
        Ok(config) => config,
        Err(error) => {
            return MatchPolicyConformanceReport {
                configured_rule_count: config.rule_count(),
                case_count: cases.len(),
                covered_rule_ids: BTreeSet::new(),
                failures: vec![format!("compile match-policy conformance config: {error}")],
            };
        }
    };
    let config = &config;
    let configured = config
        .rule_ids()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let witnessed = cases
        .iter()
        .map(|case| case.rule_id.to_owned())
        .collect::<BTreeSet<_>>();
    let mut failures = Vec::new();
    if let Err(error) = config.publish_policy_snapshot(runtime) {
        failures.push(format!("hook policy snapshot publication failed: {error}"));
    }
    if configured != witnessed {
        failures.push(format!(
            "configured and witnessed rule IDs differ: configOnly={:?} witnessOnly={:?}",
            configured.difference(&witnessed).collect::<Vec<_>>(),
            witnessed.difference(&configured).collect::<Vec<_>>()
        ));
    }
    let mut covered_rule_ids = BTreeSet::new();
    for case in &cases {
        let decision = with_synthetic_match_environment(|| {
            classify_hook_with_config(HookClassificationRequest {
                registry: runtime,
                config,
                platform,
                event: "pre-tool",
                payload: &case.payload,
            })
        });
        let actual_rule = decision
            .fields
            .get("configRuleId")
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        if actual_rule == case.rule_id
            && decision.decision == case.decision
            && decision.reason_kind == case.reason
        {
            covered_rule_ids.insert(actual_rule.to_owned());
        } else {
            let normalized_actions = decision
                .fields
                .get("normalizedActions")
                .cloned()
                .unwrap_or(Value::Null);
            failures.push(format!(
                "{}: expected rule={} decision={:?} reason={:?}; actual rule={} decision={:?} reason={:?} normalizedActions={}",
                case.name,
                case.rule_id,
                case.decision,
                case.reason,
                actual_rule,
                decision.decision,
                decision.reason_kind,
                normalized_actions,
            ));
        }
    }
    MatchPolicyConformanceReport {
        configured_rule_count: configured.len(),
        case_count: cases.len(),
        covered_rule_ids,
        failures,
    }
}

/// Reject a compiled matcher whose configured rule identities do not cover the
/// canonical production witness set. This check is intentionally runtime-free
/// so every short-lived Hook process can validate an mmap snapshot before it
/// is allowed to classify an action.
pub fn validate_match_policy_rule_coverage(config: &ClientHookConfig) -> Result<(), String> {
    let configured = config
        .rule_ids()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let witnessed = production_cases(None)
        .into_iter()
        .map(|case| case.rule_id.to_owned())
        .collect::<BTreeSet<_>>();
    validate_match_policy_rule_sets(&configured, &witnessed)
}

fn validate_match_policy_rule_sets(
    configured: &BTreeSet<String>,
    witnessed: &BTreeSet<String>,
) -> Result<(), String> {
    if configured == witnessed {
        return Ok(());
    }
    Err(format!(
        "configured and witnessed rule IDs differ: configOnly={:?} witnessOnly={:?}",
        configured.difference(&witnessed).collect::<Vec<_>>(),
        witnessed.difference(&configured).collect::<Vec<_>>()
    ))
}

#[cfg(test)]
#[path = "../tests/unit/match_policy_conformance.rs"]
mod tests;

fn production_cases(agent_search_binary: Option<&str>) -> Vec<MatchPolicyCase> {
    vec![
        MatchPolicyCase {
            name: "explicit no-agent command bypass",
            payload: shell("ASP_NO_AGENT=1 cargo test -p agent-semantic-hook"),
            rule_id: "allow-explicit-no-agent-host-bypass",
            decision: DecisionKind::Allow,
            reason: ReasonKind::None,
        },
        MatchPolicyCase {
            name: "explicit no-Agent Host bypass",
            payload: shell("ASP_NO_AGENT=1 rg -n owner src/lib.rs"),
            rule_id: "allow-explicit-no-agent-host-bypass",
            decision: DecisionKind::Allow,
            reason: ReasonKind::None,
        },
        MatchPolicyCase {
            name: "registered reasoning search",
            payload: shell("asp rust search lexical --query classify_hook --workspace ."),
            rule_id: "registered-asp-reasoning-search",
            decision: DecisionKind::Deny,
            reason: ReasonKind::SubagentReceiptRequired,
        },
        MatchPolicyCase {
            name: "testing dispatch",
            payload: shell("cargo test --workspace"),
            rule_id: "resident-testing-dispatch",
            decision: DecisionKind::Deny,
            reason: ReasonKind::SubagentReceiptRequired,
        },
        MatchPolicyCase {
            name: "raw registered source search",
            payload: json!({"tool_name":"Grep","tool_input":{"pattern":"classify_hook","path":"src/app.ts"}}),
            rule_id: "deny-raw-registered-source-search-action",
            decision: DecisionKind::Deny,
            reason: ReasonKind::RawBroadSearch,
        },
        MatchPolicyCase {
            name: "javascript inline source materialization",
            payload: shell("node -e 'require(\"fs\").readFileSync(\"src/app.ts\", \"utf8\")'"),
            rule_id: "materialize-source-access-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchPolicyCase {
            name: "python inline source materialization",
            payload: shell("python -c 'print(open(\"src/app.ts\").read())'"),
            rule_id: "materialize-source-access-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchPolicyCase {
            name: "source materialization command",
            payload: shell("sed -n '1,8p' src/app.ts"),
            rule_id: "deny-uncontrolled-source-materialization-commands",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchPolicyCase {
            name: "source search command",
            payload: shell("rg classify_hook src/app.ts"),
            rule_id: "deny-uncontrolled-source-search-commands",
            decision: DecisionKind::Deny,
            reason: ReasonKind::RawBroadSearch,
        },
        MatchPolicyCase {
            name: "git source read",
            payload: shell("git show HEAD:src/app.ts"),
            rule_id: "deny-uncontrolled-git-source-reads",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchPolicyCase {
            name: "git metadata read",
            payload: shell("git diff --check"),
            rule_id: "deny-uncontrolled-git-metadata-reads",
            decision: DecisionKind::Deny,
            reason: ReasonKind::RawBroadSearch,
        },
        MatchPolicyCase {
            name: "raw registered source execute action",
            payload: shell("read src/app.ts"),
            rule_id: "deny-raw-registered-source-action",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchPolicyCase {
            name: "registered source read materializer",
            payload: json!({"tool_name":"Read","tool_input":{"file_path":"src/app.ts"}}),
            rule_id: "route-read-to-asp-languages",
            decision: DecisionKind::Deny,
            reason: ReasonKind::DirectSourceRead,
        },
        MatchPolicyCase {
            name: "bounded JSON projection",
            payload: shell("jq -c '.package.name' package.json"),
            rule_id: "allow-bounded-json-projection",
            decision: DecisionKind::Allow,
            reason: ReasonKind::None,
        },
        MatchPolicyCase {
            name: "bounded TOML projection",
            payload: shell("yq eval '.package.name' Cargo.toml"),
            rule_id: "allow-bounded-toml-projection",
            decision: DecisionKind::Allow,
            reason: ReasonKind::None,
        },
        MatchPolicyCase {
            name: "unbounded structured projection",
            payload: shell("jq '.' package.json"),
            rule_id: "deny-unbounded-structured-projection",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchPolicyCase {
            name: "agent search JSON",
            payload: agent_search_binary
                .map(|binary| {
                    shell(&format!(
                        "{binary} search lexical projectRoot owner tests --json ."
                    ))
                })
                .unwrap_or(serde_json::Value::Null),
            rule_id: "deny-agent-search-json",
            decision: DecisionKind::Deny,
            reason: ReasonKind::AgentSearchJson,
        },
        MatchPolicyCase {
            name: "apply patch materializer",
            payload: json!({"tool_name":"apply_patch","tool_input":{"patch":"*** Begin Patch\n*** Update File: src/app.ts\n@@\n-old\n+new\n*** End Patch\n"}}),
            rule_id: "materialize-apply-patch-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::SemanticAstPatchRequired,
        },
        MatchPolicyCase {
            name: "source access materializer",
            payload: json!({"tool_name":"functions.exec_command","tool_input":{"cmd":"custom-reader '.read_text(' src/app.ts"}}),
            rule_id: "materialize-source-access-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchPolicyCase {
            name: "structured document read",
            payload: json!({"tool_name":"Read","tool_input":{"file_path":"package.json"}}),
            rule_id: "materialize-structured-document-read-action",
            decision: DecisionKind::Deny,
            reason: ReasonKind::StructuredSourceRead,
        },
    ]
}
