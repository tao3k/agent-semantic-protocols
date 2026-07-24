use agent_semantic_command_match::{BashCommandMatchV1, match_bash_command_prefix};

const HOOK_CONFIG_TEMPLATE: &str =
    include_str!("../../../agent-semantic-config/templates/hooks/config.toml");

#[derive(Debug)]
pub struct RulePrefix {
    pub rule_id: String,
    pub argv_prefix: Vec<String>,
}

fn document() -> toml::Value {
    let rendered = HOOK_CONFIG_TEMPLATE.replace("@ARGV_SOURCE_GLOBS@", "\"**/*.rs\"");
    toml::from_str::<toml::Value>(&rendered).expect("renderable hooks/config.toml template")
}

pub fn wrapper_match_enabled() -> bool {
    document()["wrapper_match"].as_str() == Some("enable")
}

pub fn rule_prefixes() -> Vec<RulePrefix> {
    let document = document();
    document["rules"]
        .as_array()
        .expect("rules array")
        .iter()
        .flat_map(|rule| {
            let rule_id = rule["id"].as_str().expect("rule id").to_string();
            ["argvPrefixAny", "argvPatternAny"]
                .into_iter()
                .flat_map(move |key| {
                    let rule_id = rule_id.clone();
                    rule.get("match")
                        .and_then(|matcher| matcher.get(key))
                        .and_then(toml::Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(move |prefix| RulePrefix {
                            rule_id: rule_id.clone(),
                            argv_prefix: prefix
                                .as_array()
                                .expect("argv match pattern")
                                .iter()
                                .map(|token| {
                                    let token = token.as_str().expect("argv token");
                                    if token == "<registered-language>" {
                                        "rust".to_string()
                                    } else {
                                        token.to_string()
                                    }
                                })
                                .collect(),
                        })
                })
        })
        .collect()
}

pub fn positive_commands(case: &RulePrefix) -> Vec<String> {
    if case.argv_prefix.len() == 1
        && matches!(
            case.argv_prefix[0].as_str(),
            "bash" | "dash" | "fish" | "sh" | "zsh"
        )
    {
        return Vec::new();
    }

    let command = format!(
        "{} --asp-match-probe crates/example/src/lib.rs",
        case.argv_prefix.join(" ")
    );
    vec![
        command.clone(),
        format!(
            "{} -p downstream-alpha -p downstream-beta",
            case.argv_prefix.join(" ")
        ),
        format!("ASP_MATCH=1 {command}"),
        format!("env ASP_MATCH=1 {command}"),
        format!("direnv exec . {command}"),
        format!("rtk {command}"),
        format!("timeout 30s {command}"),
        format!("timeout 30s direnv exec . {command}"),
        format!("printf x | {command}"),
        format!("true && {command}"),
        format!("bash -lc '{command}'"),
        format!("/opt/asp-toolchain/bin/{command}"),
        format!("CARGO_TARGET_DIR=/tmp/asp-match /opt/asp-toolchain/bin/{command}"),
        format!("direnv exec . timeout 30s /opt/asp-toolchain/bin/{command}"),
    ]
}

pub fn invalid_commands(case: &RulePrefix) -> Vec<String> {
    vec![format!("bash -lc '{} &&'", case.argv_prefix.join(" "))]
}

pub fn negative_commands(case: &RulePrefix) -> Vec<String> {
    let command = format!(
        "{} --asp-match-probe crates/example/src/lib.rs",
        case.argv_prefix.join(" ")
    );
    let mut similar = case.argv_prefix.clone();
    similar[0] = format!("not-{}", similar[0]);
    vec![
        format!("echo '{command}'"),
        format!("{} --asp-match-probe", similar.join(" ")),
    ]
}

pub fn outcome(case: &RulePrefix, command: &str) -> BashCommandMatchV1 {
    let prefix = case
        .argv_prefix
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    match_bash_command_prefix(command, &prefix)
}

pub fn assert_case(case: &RulePrefix) {
    let bare = outcome(case, &case.argv_prefix.join(" "));
    assert!(
        !matches!(&bare, BashCommandMatchV1::InvalidSyntax { .. }),
        "rule={} prefix={:?}",
        case.rule_id,
        case.argv_prefix
    );
    for command in positive_commands(case) {
        assert_eq!(
            outcome(case, &command),
            bare,
            "rule={} command={command}",
            case.rule_id
        );
    }

    for command in invalid_commands(case) {
        assert!(
            matches!(
                outcome(case, &command),
                BashCommandMatchV1::InvalidSyntax { .. }
            ),
            "rule={} command={command}",
            case.rule_id
        );
    }

    let negative = outcome(case, "asp-command-match-negative-control");
    for command in negative_commands(case) {
        assert_eq!(
            outcome(case, &command),
            negative,
            "rule={} command={command}",
            case.rule_id
        );
    }
}
