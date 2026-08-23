use super::{
    ClientHookConfig, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, temp_root,
};

#[cfg(unix)]
fn current_thread_cpu_nanos() -> u128 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `time` is a valid writable timespec and the clock is process-local.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    assert_eq!(status, 0, "read current-thread CPU clock");
    (time.tv_sec as u128) * 1_000_000_000 + (time.tv_nsec as u128)
}

#[cfg(not(unix))]
fn current_thread_cpu_nanos() -> u128 {
    static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    STARTED
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos()
}

#[test]
fn registered_reasoning_search_dispatch_survives_arbitrary_wrappers() {
    let root = temp_root("config-driven-match-engine-contract");
    let mut registry = crate::classifier::rust_registry();
    let production_text =
        include_str!("../../../../../agent-semantic-config/templates/hooks/config.toml");
    let production =
        toml::from_str::<toml::Value>(production_text).expect("production hook config");
    let config_path = root.join("config.toml");
    let default_document = agent_semantic_hook::default_client_config_template();
    let mut production_document =
        toml::from_str::<toml::Value>(&default_document).expect("production hook config document");
    production_document["rules"] = production["rules"].clone();
    fs::write(
        &config_path,
        toml::to_string(&production_document).expect("serialize production hook config"),
    )
    .expect("write production hook config");
    let production_config = load_client_config(&config_path).expect("load production hook config");
    let baseline_config_path = root.join("baseline-config.toml");
    let mut baseline_document =
        toml::from_str::<toml::Value>(&default_document).expect("baseline hook config document");
    baseline_document["rules"] = toml::Value::Array(Vec::new());
    fs::write(
        &baseline_config_path,
        toml::to_string(&baseline_document).expect("serialize baseline hook config"),
    )
    .expect("write baseline hook config");
    let baseline_config =
        load_client_config(&baseline_config_path).expect("load baseline hook config");
    let matrix = toml::from_str::<toml::Value>(include_str!(
        "../../../../../agent-semantic-config/templates/hooks/config-test.toml"
    ))
    .expect("hook match config test");
    let matrix_schema = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../../../schemas/semantic-hook-match-config-test.v1.schema.json"
    ))
    .expect("hook match config-test schema");
    let matrix_json = serde_json::to_value(&matrix).expect("config-test JSON projection");
    assert!(
        jsonschema::validator_for(&matrix_schema)
            .expect("hook match config-test validator")
            .is_valid(&matrix_json),
        "{matrix_json}"
    );
    let max_matcher_micros = matrix["performance"]["maxMatcherMicros"]
        .as_integer()
        .expect("matcher latency gate") as u128;

    let production_rules = production["rules"].as_array().expect("production rules");
    let matrix_rules = matrix["rules"].as_array().expect("config-test rules");
    let production_rule_ids = production_rules
        .iter()
        .map(|rule| rule["id"].as_str().expect("production rule id"))
        .collect::<std::collections::BTreeSet<_>>();
    let matrix_rule_ids = matrix_rules
        .iter()
        .map(|rule| rule["id"].as_str().expect("config-test rule id"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        matrix_rule_ids, production_rule_ids,
        "config-test.toml must cover every production rule exactly once"
    );
    assert_eq!(
        matrix_rule_ids.len(),
        matrix_rules.len(),
        "config-test.toml contains duplicate rule ids"
    );
    let wrapper_templates = matrix["wrapperTemplates"]
        .as_array()
        .expect("wrapper templates");
    let render_wrapper = |template: &str, command: &str| {
        if template.contains("\"{command}\"") {
            let escaped = command
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace('`', "\\`");
            return template.replace("{command}", &escaped);
        }
        if template.contains("'{command}'") {
            return template.replace("{command}", &command.replace('\'', "'\"'\"'"));
        }
        template.replace("{command}", command)
    };
    let external_evidence_rule_ids = production_rules
        .iter()
        .filter(|rule| {
            let match_config = rule.get("match");
            match_config
                .and_then(|config| config.get("structuredProjection"))
                .is_some()
                || match_config
                    .and_then(|config| config.get("argvWorkspaceRegularFile"))
                    .and_then(toml::Value::as_bool)
                    == Some(true)
                || match_config
                    .and_then(|config| config.get("argvRegisteredSourceFile"))
                    .and_then(toml::Value::as_bool)
                    == Some(true)
        })
        .map(|rule| rule["id"].as_str().expect("materialized rule id"))
        .collect::<std::collections::BTreeSet<_>>();
    let executable_is_available = |binary: &str| {
        let binary_path = std::path::Path::new(binary);
        if binary_path.is_absolute() || binary.contains('/') {
            return binary_path.is_file();
        }
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path)
                .map(|directory| directory.join(binary))
                .any(|candidate| candidate.is_file())
        })
    };
    let unavailable_projection_binaries = production_rules
        .iter()
        .filter_map(|rule| {
            let binary = rule
                .get("match")
                .and_then(|match_config| match_config.get("structuredProjection"))
                .and_then(|projection| projection.get("binary"))
                .and_then(toml::Value::as_str)?;
            if executable_is_available(binary) {
                return None;
            }
            assert_eq!(
                rule["fields"]["capabilityActivation"].as_str(),
                Some("lazy-executable"),
                "{} has an unavailable structured projection binary without lazy activation",
                rule["id"].as_str().expect("production rule id")
            );
            Some(binary)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let workspace_cwd = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("hook matcher workspace root")
        .to_string_lossy()
        .into_owned();
    registry.project_root = workspace_cwd.clone();
    assert!(
        std::path::Path::new(&registry.project_root)
            .join("schemas/semantic-hook-match-scenario.v1.schema.json")
            .is_file(),
        "hook matcher fixture root does not contain the structured projection scenario: {}",
        registry.project_root
    );
    let case_sequence = std::cell::Cell::new(0_u64);
    let measured_matcher_shapes = std::cell::RefCell::new(std::collections::BTreeSet::new());
    let run_case = |tool_name: &str, tool_input: serde_json::Value, case_id: &str| {
        let case_index = case_sequence.get();
        case_sequence.set(case_index + 1);
        let classify_sample = |config: &ClientHookConfig, sample: &str| {
            let payload = json!({
                "session_id": format!("config-driven-match-engine-contract:{case_index}:{case_id}:{sample}"),
                "cwd": workspace_cwd,
                "tool_name": tool_name,
                "tool_input": tool_input
            });
            classify_hook_with_config(HookClassificationRequest {
                registry: &registry,
                config,
                platform: "codex",
                event: "pre-tool",
                payload: &payload,
            })
        };
        let decision = classify_sample(&production_config, "decision");
        let rule_id = case_id.split(':').next().expect("case rule id");
        let shape = if case_id.contains(":wrapper:") {
            "wrapper"
        } else {
            "direct"
        };
        if !measured_matcher_shapes
            .borrow_mut()
            .insert((rule_id.to_owned(), shape))
        {
            return decision;
        }
        // Keep the conformance matrix broad while bounding its aggregate wall
        // time under Cargo's default parallel test scheduler. Correctness still
        // covers every witness; latency is sampled once per rule and envelope
        // shape so repeated wrapper strings do not multiply the wall budget.
        const MATCHER_SAMPLE_BATCH: u128 = 8;
        let mut elapsed_samples = [0_u128; 5];
        for (batch_index, elapsed_micros) in elapsed_samples.iter_mut().enumerate() {
            let measure = |config: &ClientHookConfig, label: &str| {
                let started = current_thread_cpu_nanos();
                for sample_index in 0..MATCHER_SAMPLE_BATCH {
                    let _ = classify_sample(
                        config,
                        &format!("performance-{batch_index}-{label}-{sample_index}"),
                    );
                }
                (current_thread_cpu_nanos() - started) / 1_000 / MATCHER_SAMPLE_BATCH
            };
            let (production_micros, baseline_micros) = if batch_index % 2 == 0 {
                let baseline = measure(&baseline_config, "baseline");
                let production = measure(&production_config, "production");
                (production, baseline)
            } else {
                let production = measure(&production_config, "production");
                let baseline = measure(&baseline_config, "baseline");
                (production, baseline)
            };
            *elapsed_micros = production_micros.saturating_sub(baseline_micros);
        }
        elapsed_samples.sort_unstable();
        let elapsed_micros = elapsed_samples[0];
        if !external_evidence_rule_ids.contains(rule_id) {
            assert!(
                elapsed_micros <= max_matcher_micros,
                "matcher exceeded config-test.toml current-thread CPU gate: case={case_id} bestBatchMeanMicros={elapsed_micros} samples={elapsed_samples:?} maxMatcherMicros={max_matcher_micros}"
            );
        }
        decision
    };

    for rule in matrix_rules {
        let rule_id = rule["id"].as_str().expect("config-test rule id");
        let production_rule = production_rules
            .iter()
            .find(|candidate| candidate["id"].as_str() == Some(rule_id))
            .expect("covered production rule");
        let expected_decision = production_rule["decision"]
            .as_str()
            .expect("production decision");
        let expected_reason = production_rule
            .get("reasonKind")
            .and_then(toml::Value::as_str);
        let projection_binary = production_rule
            .get("match")
            .and_then(|match_config| match_config.get("structuredProjection"))
            .and_then(|projection| projection.get("binary"))
            .and_then(toml::Value::as_str);
        let projection_capability_available = projection_binary
            .is_none_or(|binary| !unavailable_projection_binaries.contains(binary));
        let positive_commands = rule.get("positiveCommands").and_then(toml::Value::as_array);
        let negative_commands = rule.get("negativeCommands").and_then(toml::Value::as_array);
        let positive_tools = rule.get("positiveTools").and_then(toml::Value::as_array);
        let negative_tools = rule.get("negativeTools").and_then(toml::Value::as_array);
        let wrapper_exemption = rule.get("wrapperExemption").and_then(toml::Value::as_str);
        assert!(
            positive_commands.is_some() || positive_tools.is_some(),
            "{rule_id} has no positive cases"
        );
        assert!(
            negative_commands.is_some() || negative_tools.is_some(),
            "{rule_id} has no negative cases"
        );

        macro_rules! assert_positive {
            ($decision:expr, $case_id:expr, $expected_command:expr) => {{
                let decision = $decision;
                let case_id = $case_id;
                let expected_command: Option<&str> = $expected_command;
                let decision_json =
                    serde_json::to_value(decision).expect("serialize hook decision");
                assert_eq!(
                    decision_json["fields"]["configRuleId"].as_str(),
                    Some(rule_id),
                    "positive case selected the wrong rule: {case_id} rule={rule:#?} productionRule={production_rule:#?} decision={decision:#?}"
                );
                assert_eq!(
                    decision_json["decision"]
                        .as_str()
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some(expected_decision),
                    "positive case selected the wrong decision: {case_id}"
                );
                if let Some(expected_reason) = expected_reason {
                    assert_eq!(
                        decision_json["reasonKind"].as_str(),
                        Some(expected_reason),
                        "positive case selected the wrong reason: {case_id}"
                    );
                }
                if production_rule.get("dispatch").is_some() {
                    assert_eq!(
                        decision_json["fields"]["choicePlaneOwner"].as_str(),
                        Some("org-contract:agent-interactive"),
                        "positive case lost the Org ChoicePlane: {case_id}"
                    );
                    assert_eq!(
                        decision_json["fields"]["agentWindowCommand"].as_str(),
                        Some("asp session --agents choice-plane"),
                        "positive case changed the Org Agent window: {case_id}"
                    );
                    let target_role = decision_json["fields"]["targetAgentRole"]
                        .as_str()
                        .filter(|role| !role.is_empty())
                        .expect("positive case omitted ChoicePlane target role");
                    let receipt_kind = decision_json["fields"]["receiptKind"]
                        .as_str()
                        .filter(|receipt| !receipt.is_empty())
                        .expect("positive case omitted ChoicePlane receipt kind");
                    assert_eq!(
                        decision_json["fields"]["agentSessionAction"].as_str(),
                        Some("dispatch-choice-plane-role")
                    );
                    if decision_json["reasonKind"].as_str()
                        == Some("subagent-receipt-required")
                    {
                        let expected_message = format!(
                            "role `{target_role}` and require receipt `{receipt_kind}`"
                        );
        let actual_message = decision_json["message"]
            .as_str()
            .expect("positive dispatch message");
        if decision_json["fields"]["configRuleId"].as_str()
            == Some("registered-asp-reasoning-search")
        {
            assert!(
                actual_message.starts_with(
                    "Raw search over registered source is denied only in the current Agent."
                ),
                "registered search lost the Config TOML policy message: {case_id}: {actual_message}"
            );
        }
        assert!(
            actual_message.contains(&expected_message),
            "positive case lost the shared dynamic dispatch guidance: {case_id}: {actual_message}"
        );
                    }
                    for forbidden in [
                        "residentName",
                        "targetAgentName",
                        "targetAgentKind",
                        "targetAgentDisplayRole",
                        "targetAgentDescription",
                    ] {
                        assert!(
                            decision_json["fields"].get(forbidden).is_none(),
                            "positive case preselected a resident through {forbidden}: {case_id}"
                        );
                    }
                }
                if let Some(expected_command) = expected_command {
                    assert_eq!(
                        decision_json["subject"]["command"].as_str(),
                        Some(expected_command),
                        "typed dispatch changed the command subject: {case_id}"
                    );
                    assert!(
                        decision_json.get("interactiveCommand").is_none(),
                        "Hook must not materialize a Rust-owned ChoicePlane: {case_id}"
                    );
                }
            }};
        }

        if projection_capability_available {
            for command in positive_commands
                .into_iter()
                .flatten()
                .map(|value| value.as_str().expect("positive command"))
            {
                let decision = run_case(
                    "Bash",
                    json!({"command": command}),
                    &format!("{rule_id}:positive"),
                );
                assert_positive!(&decision, command, Some(command));
                if wrapper_exemption.is_none() {
                    for wrapper in wrapper_templates {
                        let wrapped =
                            render_wrapper(wrapper.as_str().expect("wrapper template"), command);
                        let decision = run_case(
                            "Bash",
                            json!({"command": wrapped}),
                            &format!("{rule_id}:wrapper:{wrapped}"),
                        );
                        assert_positive!(&decision, &wrapped, Some(&wrapped));
                    }
                }
            }
        }
        if positive_commands.is_none() {
            assert!(
                wrapper_exemption.is_some(),
                "{rule_id} must explain why wrappers do not apply"
            );
        }
        for command in negative_commands
            .into_iter()
            .flatten()
            .map(|value| value.as_str().expect("negative command"))
        {
            let requires_unavailable_projection =
                unavailable_projection_binaries.iter().any(|binary| {
                    matches!(
                        agent_semantic_shell_parser::match_bash_wrapped_command_prefix(
                            command,
                            &[*binary],
                        ),
                        agent_semantic_shell_parser::BashCommandMatch::Parsed(
                            agent_semantic_shell_parser::PrefixMatch::Matched
                        )
                    )
                });
            if requires_unavailable_projection {
                continue;
            }
            let decision = run_case(
                "Bash",
                json!({"command": command}),
                &format!("{rule_id}:negative"),
            );
            assert_ne!(
                decision
                    .fields
                    .get("configRuleId")
                    .and_then(serde_json::Value::as_str),
                Some(rule_id),
                "negative case unexpectedly selected {rule_id}: {command}"
            );
        }
        if projection_capability_available {
            for tool_case in positive_tools.into_iter().flatten() {
                let tool_name = tool_case["name"].as_str().expect("positive tool name");
                let tool_input =
                    serde_json::to_value(&tool_case["input"]).expect("positive tool input");
                let decision = run_case(tool_name, tool_input, &format!("{rule_id}:positive-tool"));
                assert_positive!(&decision, tool_name, None);
            }
        }
        for tool_case in negative_tools.into_iter().flatten() {
            let tool_name = tool_case["name"].as_str().expect("negative tool name");
            let tool_input =
                serde_json::to_value(&tool_case["input"]).expect("negative tool input");
            let decision = run_case(tool_name, tool_input, &format!("{rule_id}:negative-tool"));
            assert_ne!(
                decision
                    .fields
                    .get("configRuleId")
                    .and_then(serde_json::Value::as_str),
                Some(rule_id),
                "negative tool case unexpectedly selected {rule_id}: {tool_name}"
            );
        }
    }

    let _ = fs::remove_dir_all(root);
}
