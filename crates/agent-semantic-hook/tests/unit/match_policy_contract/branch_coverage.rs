use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde_json::{Value, json};

use super::{
    CAPABILITY_CHILD_ENV, classify, default_client_config_template, load_client_config_for_project,
    registry, run_with_projection_capabilities_for, shell, temp_project_root,
};

#[path = "branch_coverage/scenarios.rs"]
mod scenarios;

const MATCH_ALTERNATIVE_FIELDS: &[&str] = &[
    "argvPatternAny",
    "commandProfileAny",
    "actionAny",
    "effectAny",
    "subjectAny",
    "subjectKindAny",
    "authorityAny",
    "effectRules",
    "authorityRules",
    "commandContainsAny",
    "argvPrefixAny",
    "commandAny",
    "argvWorkspaceRegularFile",
    "argvRegisteredSourceFile",
    "binary",
    "documentFormat",
    "filterGrammar",
    "optionalSubcommandAny",
    "optionAny",
    "optionValueArity",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AtomKind {
    Predicate,
    Derive,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CoverageKey {
    rule_id: String,
    matcher: String,
    alternative: String,
    kind: AtomKind,
}

impl CoverageKey {
    fn receipt(&self) -> String {
        format!(
            "rule={}|kind={:?}|matcher={}|alt={}",
            self.rule_id, self.kind, self.matcher, self.alternative
        )
    }
}

#[derive(Debug)]
struct ProductionPolicy {
    atoms: BTreeSet<CoverageKey>,
    rules: BTreeSet<String>,
    profiles: BTreeMap<(String, String), Vec<String>>,
}

fn canonical_scalar(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => value.clone(),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => value.to_string(),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Datetime(value) => value.to_string(),
        toml::Value::Array(values) => values
            .iter()
            .map(canonical_scalar)
            .collect::<Vec<_>>()
            .join(" "),
        toml::Value::Table(table) => table
            .iter()
            .map(|(key, value)| format!("{key}={}", canonical_scalar(value)))
            .collect::<Vec<_>>()
            .join(","),
    }
}

fn alternative_values(value: &toml::Value) -> Vec<String> {
    match value {
        toml::Value::Array(values) => values.iter().map(canonical_scalar).collect(),
        toml::Value::Table(values) => values
            .iter()
            .map(|(key, value)| format!("{key}={}", canonical_scalar(value)))
            .collect(),
        value => vec![canonical_scalar(value)],
    }
}

fn looks_like_rule(table: &toml::map::Map<String, toml::Value>) -> bool {
    table.get("id").and_then(toml::Value::as_str).is_some()
        && (table.contains_key("match")
            || table.contains_key("decisionMaterializer")
            || table.contains_key("reasonKind"))
}

fn collect_rule_tables<'a>(
    value: &'a toml::Value,
    output: &mut Vec<&'a toml::map::Map<String, toml::Value>>,
) {
    match value {
        toml::Value::Table(table) => {
            if looks_like_rule(table) {
                output.push(table);
            }
            for child in table.values() {
                collect_rule_tables(child, output);
            }
        }
        toml::Value::Array(values) => {
            for child in values {
                collect_rule_tables(child, output);
            }
        }
        _ => {}
    }
}

fn collect_fields(
    rule_id: &str,
    value: &toml::Value,
    path: &str,
    atoms: &mut BTreeSet<CoverageKey>,
) {
    match value {
        toml::Value::Table(table) => {
            for (field, child) in table {
                let child_path = if path.is_empty() {
                    field.clone()
                } else {
                    format!("{path}.{field}")
                };
                if MATCH_ALTERNATIVE_FIELDS.contains(&field.as_str()) {
                    let kind = if matches!(field.as_str(), "effectRules" | "authorityRules") {
                        AtomKind::Derive
                    } else {
                        AtomKind::Predicate
                    };
                    for alternative in alternative_values(child) {
                        atoms.insert(CoverageKey {
                            rule_id: rule_id.to_owned(),
                            matcher: child_path.clone(),
                            alternative,
                            kind,
                        });
                    }
                }
                collect_fields(rule_id, child, &child_path, atoms);
            }
        }
        toml::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_fields(rule_id, child, &format!("{path}[{index}]"), atoms);
            }
        }
        _ => {}
    }
}

fn collect_command_profiles(
    value: &toml::Value,
    profiles: &mut BTreeMap<(String, String), Vec<String>>,
) {
    match value {
        toml::Value::Table(table) => {
            if let Some(command_profiles) =
                table.get("commandProfiles").and_then(toml::Value::as_array)
            {
                for profile in command_profiles {
                    let Some(profile) = profile.as_table() else {
                        continue;
                    };
                    let Some(profile_id) = profile.get("id").and_then(toml::Value::as_str) else {
                        continue;
                    };
                    let Some(categories) =
                        profile.get("categories").and_then(toml::Value::as_table)
                    else {
                        continue;
                    };
                    for (category, prefixes) in categories {
                        profiles.insert(
                            (profile_id.to_owned(), category.clone()),
                            alternative_values(prefixes),
                        );
                    }
                }
            }
            for child in table.values() {
                collect_command_profiles(child, profiles);
            }
        }
        toml::Value::Array(values) => {
            for child in values {
                collect_command_profiles(child, profiles);
            }
        }
        _ => {}
    }
}

fn parse_production_policy() -> ProductionPolicy {
    let template = default_client_config_template();
    let document = template
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .expect("production hook config template must parse as TOML");
    let mut rule_tables = Vec::new();
    collect_rule_tables(&document, &mut rule_tables);

    let mut atoms = BTreeSet::new();
    let mut rules = BTreeSet::new();
    let mut profiles = BTreeMap::new();
    collect_command_profiles(&document, &mut profiles);

    for table in rule_tables {
        let rule_id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .expect("rule table has string id");
        rules.insert(rule_id.to_owned());
        if let Some(match_value) = table.get("match") {
            collect_fields(rule_id, match_value, "match", &mut atoms);
        }

        if let Some(profile_names) = table
            .get("match")
            .and_then(toml::Value::as_table)
            .and_then(|match_table| match_table.get("commandProfileAny"))
            .and_then(toml::Value::as_array)
        {
            for profile_match in profile_names {
                let profile_match = profile_match
                    .as_table()
                    .expect("commandProfileAny entries must be inline tables");
                let profile_name = profile_match
                    .get("profile")
                    .and_then(toml::Value::as_str)
                    .expect("command profile match must name profile");
                let category = profile_match
                    .get("category")
                    .and_then(toml::Value::as_str)
                    .expect("command profile match must name category");
                let prefixes = profiles
                    .get(&(profile_name.to_owned(), category.to_owned()))
                    .unwrap_or_else(|| {
                    panic!(
                        "rule {rule_id} references missing command profile {profile_name}/{category}; known={:?}",
                        profiles.keys().collect::<Vec<_>>()
                    )
                });
                for prefix in prefixes {
                    atoms.insert(CoverageKey {
                        rule_id: rule_id.to_owned(),
                        matcher: format!("commandProfiles.{profile_name}.{category}"),
                        alternative: prefix.clone(),
                        kind: AtomKind::Predicate,
                    });
                }
            }
        }
    }

    ProductionPolicy {
        atoms,
        rules,
        profiles,
    }
}

#[test]
fn production_branch_atom_inventory_is_derived_from_the_template() {
    let policy = parse_production_policy();
    assert_eq!(
        policy.rules.len(),
        16,
        "production rule discovery drifted: {:?}",
        policy.rules
    );
    assert!(
        policy.atoms.len() >= 150,
        "branch inventory is unexpectedly shallow ({} atoms):\n{}",
        policy.atoms.len(),
        policy
            .atoms
            .iter()
            .map(CoverageKey::receipt)
            .collect::<Vec<_>>()
            .join("\n")
    );
    eprintln!(
        "match-policy branch inventory: rules={} atoms={}",
        policy.rules.len(),
        policy.atoms.len()
    );
}

fn shell_surface(tool_name: &str, field: &str, command: &str) -> Value {
    json!({
        "tool_name": tool_name,
        "tool_input": {field: command},
    })
}

fn value_after<'a>(alternative: &'a str, name: &str) -> Option<&'a str> {
    alternative.split(',').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == name).then_some(value)
    })
}

fn source_path_for(alternative: &str) -> &'static str {
    if alternative == "registered-language-source-pattern" {
        "src/*.ts"
    } else {
        "src/app.ts"
    }
}

fn command_for_atom(atom: &CoverageKey, policy: &ProductionPolicy) -> String {
    let alt = atom.alternative.as_str();
    let matcher = atom.matcher.as_str();
    match atom.rule_id.as_str() {
        "registered-asp-reasoning-search" => {
            let prefix = alt.replace("<registered-language>", "rust");
            if prefix.contains("--term") {
                format!("{prefix} classify_hook --workspace .")
            } else {
                format!("{prefix} lexical --query classify_hook --workspace .")
            }
        }
        "resident-testing-dispatch" => {
            if matcher == "match.commandProfileAny" {
                let profile = value_after(alt, "profile").expect("profile alternative");
                let category = value_after(alt, "category").expect("category alternative");
                policy
                    .profiles
                    .get(&(profile.to_owned(), category.to_owned()))
                    .and_then(|prefixes| prefixes.first())
                    .cloned()
                    .expect("profile alternative has command prefix")
            } else {
                alt.to_owned()
            }
        }
        "deny-raw-registered-source-search-action" => {
            format!("grep classify_hook {}", source_path_for(alt))
        }
        "deny-raw-registered-source-action" => {
            let prefix = value_after(alt, "argvPrefix").unwrap_or("read");
            match prefix {
                "cp" | "mv" => format!("{prefix} src/app.ts src/app2.ts"),
                "git mv" => "git mv src/app.ts src/app2.ts".to_owned(),
                "git show" => "git show HEAD:src/app.ts".to_owned(),
                "asp <registered-language>" => {
                    "asp rust query --term classify_hook --workspace .".to_owned()
                }
                _ => format!("{prefix} {}", source_path_for(alt)),
            }
        }
        "deny-agent-search-json" => {
            format!("ts-harness search lexical projectRoot owner tests {alt} .")
        }
        "materialize-registered-source-read-action" => {
            format!("read {}", source_path_for(alt))
        }
        "materialize-source-access-policy" => {
            let contains = value_after(alt, "commandContainsAny").unwrap_or(alt);
            format!("custom-reader '{contains}' src/app.ts")
        }
        "deny-uncontrolled-source-search-commands" => {
            format!("{alt} classify_hook src/app.ts")
        }
        "allow-bounded-json-projection" => match matcher {
            path if path.ends_with("optionAny") => {
                format!("jq {alt} '.package.name' package.json")
            }
            path if path.ends_with("optionValueArity") => {
                let (option, arity) = alt.split_once('=').expect("option arity alternative");
                let arity = arity.parse::<usize>().expect("integer option arity");
                let values = (0..arity)
                    .map(|index| format!("value{index}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("jq {option} {values} '.package.name' package.json")
            }
            _ => "jq -c '.package.name' package.json".to_owned(),
        },
        "allow-bounded-toml-projection" => {
            if matcher.ends_with("optionalSubcommandAny") {
                format!("yq {alt} '.package.name' Cargo.toml")
            } else {
                "yq eval '.package.name' Cargo.toml".to_owned()
            }
        }
        "deny-unbounded-structured-projection" => {
            let binary = if matcher.ends_with("commandAny") {
                alt
            } else {
                "jq"
            };
            let file = if binary == "yq" {
                "Cargo.toml"
            } else {
                "package.json"
            };
            format!("{binary} '.' {file}")
        }
        "deny-uncontrolled-source-materialization-commands" => {
            let prefix = if matcher.ends_with("argvPrefixAny") {
                alt
            } else if atom.kind == AtomKind::Derive {
                value_after(alt, "argvPrefix").unwrap_or("sed")
            } else {
                "sed"
            };
            if prefix == "sed" {
                format!("sed -n '1,8p' {}", source_path_for(alt))
            } else {
                format!("{prefix} {}", source_path_for(alt))
            }
        }
        "deny-uncontrolled-python-inline-source-materialization" => {
            let prefix = if matcher.ends_with("argvPrefixAny") {
                alt
            } else {
                "python"
            };
            let contains = if matcher.ends_with("commandContainsAny") {
                alt
            } else {
                ".read_text("
            };
            format!("{prefix} -c 'from pathlib import Path; print(Path(\"src/app.ts\"){contains})'")
        }
        "deny-uncontrolled-javascript-inline-source-materialization" => {
            let prefix = if matcher.ends_with("argvPrefixAny") {
                alt
            } else {
                "node"
            };
            let contains = if matcher.ends_with("commandContainsAny") {
                alt
            } else {
                "readFileSync("
            };
            format!("{prefix} -e 'require(\"fs\").{contains}\"src/app.ts\")'")
        }
        "deny-uncontrolled-git-source-reads" => {
            format!("{alt} HEAD:src/app.ts")
        }
        other => panic!(
            "no atom witness command builder for {other}: {}",
            atom.receipt()
        ),
    }
}

fn payload_for_atom(atom: &CoverageKey, policy: &ProductionPolicy) -> Value {
    let command = command_for_atom(atom, policy);
    if atom.matcher.ends_with("authorityAny") {
        return match atom.alternative.as_str() {
            "raw-host-action" => json!({
                "tool_name": "execute",
                "tool_input": {"path": "src/app.ts", "command": command},
            }),
            "unknown" => json!({
                "tool_name": "mystery_execute",
                "tool_input": {"path": "src/app.ts", "command": command},
            }),
            _ => shell_surface("exec_command", "cmd", &command),
        };
    }
    match atom.rule_id.as_str() {
        "deny-raw-registered-source-search-action" => json!({
            "tool_name": "Grep",
            "tool_input": {
                "pattern": "classify_hook",
                "path": source_path_for(&atom.alternative),
            },
        }),
        "materialize-registered-source-read-action" => json!({
            "tool_name": "Read",
            "tool_input": {"file_path": source_path_for(&atom.alternative)},
        }),
        _ => shell_surface("exec_command", "cmd", &command),
    }
}

fn expected_winner(atom: &CoverageKey) -> Option<&str> {
    if atom.kind == AtomKind::Derive {
        return None;
    }
    Some(atom.rule_id.as_str())
}

fn requires_low_level_derivation_witness(atom: &CoverageKey) -> bool {
    atom.rule_id == "deny-raw-registered-source-action"
        && atom.kind == AtomKind::Derive
        && (atom.matcher.ends_with("authorityRules")
            || atom.alternative.contains("effect=edit")
            || atom.alternative.contains("argvPrefix=git show"))
}

fn normalized_agent_action(decision: &agent_semantic_hook::HookDecision) -> &Value {
    decision.fields.get("agentAction").unwrap_or(&Value::Null)
}

fn validate_atom_witness(
    atom: &CoverageKey,
    decision: &agent_semantic_hook::HookDecision,
) -> Result<(), String> {
    let actual_rule = decision.fields.get("configRuleId").and_then(Value::as_str);
    if let Some(expected_rule) = expected_winner(atom)
        && actual_rule != Some(expected_rule)
    {
        return Err(format!(
            "{} expected winner {expected_rule}, actual={actual_rule:?}, action={}, normalized={}",
            atom.receipt(),
            normalized_agent_action(decision),
            decision
                .fields
                .get("normalizedActions")
                .unwrap_or(&Value::Null)
        ));
    }

    let action = normalized_agent_action(decision);
    let expected_envelope = if atom.matcher.ends_with("actionAny") {
        Some(("action", atom.alternative.as_str()))
    } else if atom.matcher.ends_with("effectAny") {
        Some(("effect", atom.alternative.as_str()))
    } else if atom.matcher.ends_with("authorityAny") {
        Some(("authority", atom.alternative.as_str()))
    } else if atom.matcher.ends_with("subjectKindAny") {
        let subjects = action
            .get("subjects")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !subjects.iter().any(|subject| {
            subject.get("kind").and_then(Value::as_str) == Some(atom.alternative.as_str())
        }) {
            return Err(format!(
                "{} missing subject-kind envelope; action={action}",
                atom.receipt()
            ));
        }
        None
    } else if atom.kind == AtomKind::Derive && atom.matcher.ends_with("effectRules") {
        Some((
            "effect",
            value_after(&atom.alternative, "effect").expect("effect-rule value"),
        ))
    } else if atom.kind == AtomKind::Derive && atom.matcher.ends_with("authorityRules") {
        Some((
            "authority",
            value_after(&atom.alternative, "authority").expect("authority-rule value"),
        ))
    } else {
        None
    };
    if let Some((field, expected)) = expected_envelope
        && action.get(field).and_then(Value::as_str) != Some(expected)
    {
        return Err(format!(
            "{} missing {field}={expected} envelope; action={action}",
            atom.receipt()
        ));
    }
    Ok(())
}

#[test]
fn every_production_branch_atom_has_a_classifier_witness() {
    const TEST_NAME: &str = "match_policy_contract::branch_coverage::every_production_branch_atom_has_a_classifier_witness";
    if std::env::var_os(CAPABILITY_CHILD_ENV).is_none() {
        run_with_projection_capabilities_for(TEST_NAME);
        return;
    }

    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).expect("create branch contract source root");
    fs::write(root.join("src/app.ts"), "export const value = 1;\n")
        .expect("write registered source fixture");
    fs::write(
        root.join("package.json"),
        "{\"package\":{\"name\":\"hook\"}}\n",
    )
    .expect("write JSON projection fixture");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"hook\"\n")
        .expect("write TOML projection fixture");
    let config_path = root.join("config.toml");
    fs::write(&config_path, default_client_config_template())
        .expect("write production hook config");
    let config =
        load_client_config_for_project(&config_path, &root).expect("load production hook config");
    let mut runtime = registry();
    runtime.project_root = root.to_string_lossy().into_owned();
    let policy = parse_production_policy();
    let classifier_expected = policy
        .atoms
        .iter()
        .filter(|atom| !requires_low_level_derivation_witness(atom))
        .cloned()
        .collect::<BTreeSet<_>>();
    let low_level_expected = policy
        .atoms
        .iter()
        .filter(|atom| requires_low_level_derivation_witness(atom))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        low_level_expected.len(),
        5,
        "low-level derivation witness delegation drifted:\n{}",
        low_level_expected
            .iter()
            .map(CoverageKey::receipt)
            .collect::<Vec<_>>()
            .join("\n")
    );

    let mut witnessed = BTreeSet::new();
    let mut failures = Vec::new();
    for atom in &classifier_expected {
        let payload = payload_for_atom(atom, &policy);
        let decision = classify(&runtime, &config, &payload);
        match validate_atom_witness(atom, &decision) {
            Ok(()) => {
                witnessed.insert(atom.clone());
            }
            Err(error) => failures.push(format!("{error}; payload={payload}")),
        }
    }
    let missing = classifier_expected
        .difference(&witnessed)
        .map(CoverageKey::receipt)
        .collect::<Vec<_>>();
    fs::remove_dir_all(root).expect("cleanup branch contract root");
    assert!(
        failures.is_empty() && missing.is_empty(),
        "production branch witness contract failed: classifierExpected={} classifierWitnessed={} lowLevelExpected={} missing={}\n{}\nmissing keys:\n{}",
        classifier_expected.len(),
        witnessed.len(),
        low_level_expected.len(),
        missing.len(),
        failures.join("\n"),
        missing.join("\n")
    );
    eprintln!(
        "match-policy branch witnesses: total={} classifier={} lowLevel={} missing=0",
        policy.atoms.len(),
        witnessed.len(),
        low_level_expected.len()
    );
}
