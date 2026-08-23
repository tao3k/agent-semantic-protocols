use super::ClientHookConfig;

#[test]
fn complete_durable_hook_artifact_recovery_has_bounded_typical_and_hard_latency() {
    let live = ClientHookConfig::default();
    let bytes = live
        .durable_snapshot_config()
        .to_binary_bytes()
        .expect("encode complete Binary v1 Hook matcher artifact");
    assert!(
        bytes.len() < 4 * 1024 * 1024,
        "complete durable Hook matcher artifact must fit the Runtime Server seqlock memory; observed {} bytes",
        bytes.len()
    );
    let mut samples = (0..100)
        .map(|_| {
            let started = std::time::Instant::now();
            let artifact = super::DurableHookConfigArtifact::from_binary_bytes(&bytes)
                .expect("decode complete Binary v1 Hook matcher artifact");
            let config = ClientHookConfig::from_durable_snapshot_config(artifact)
                .expect("hydrate complete durable Hook matcher artifact");
            std::hint::black_box(config.rule_count());
            started.elapsed()
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
    let max = *samples.last().expect("durable recovery samples");
    eprintln!(
        "[hook-durable-artifact-recovery] bytes={} rules={} admissions=100 p95Nanos={} maxNanos={} typicalBudgetNanos=50000000 hardBudgetNanos=500000000",
        bytes.len(),
        live.rule_count(),
        p95.as_nanos(),
        max.as_nanos()
    );
    assert!(
        p95 < std::time::Duration::from_millis(50),
        "one-time durable Hook generation recovery p95 exceeded 50ms: {p95:?}"
    );
    assert!(
        max < std::time::Duration::from_millis(500),
        "one-time durable Hook generation recovery exceeded the 500ms hard boundary: {max:?}"
    );
}

#[test]
fn binary_durable_hook_artifact_round_trips_without_json_or_base64() {
    let live = ClientHookConfig::default();
    let durable = live.durable_snapshot_config();
    eprintln!(
        "binaryHookConfigJsonBytes={} matcherPostcardBytes={} providerJsonBytes={}",
        serde_json::to_vec(&durable.config)
            .expect("config json")
            .len(),
        postcard::to_allocvec(&durable.rule_matchers)
            .expect("matcher postcard")
            .len(),
        serde_json::to_vec(&durable.provider_projections)
            .expect("provider json")
            .len(),
    );
    let bytes = durable
        .to_binary_bytes()
        .expect("encode binary durable Hook matcher artifact");
    let shards = live
        .durable_direct_read_decision_shards()
        .expect("compile policy-derived direct-read shards");
    assert!(!shards.is_empty());
    let largest_shard = shards
        .iter()
        .map(|(_, _, shard)| shard.len())
        .max()
        .expect("direct-read shard size");
    eprintln!("binaryHookLargestDirectReadShardBytes={largest_shard}");
    assert!(
        largest_shard < bytes.len(),
        "direct-read shard must be smaller than the complete matcher"
    );
    for (_, placeholder, shard) in &shards {
        let mut decision = crate::HookDecision::from_compact_binary(shard)
            .expect("decode compact direct-read decision shard");
        assert!(decision.replace_template_marker(placeholder, "generated/replaced.rs"));
    }
    eprintln!("binaryHookArtifactBytes={}", bytes.len());
    let decode_started = std::time::Instant::now();
    let artifact = super::DurableHookConfigArtifact::from_binary_bytes(&bytes)
        .expect("decode binary durable Hook matcher artifact");
    eprintln!(
        "binaryHookArtifactDecodeMicros={}",
        decode_started.elapsed().as_micros()
    );
    let hydrate_started = std::time::Instant::now();
    let recovered = ClientHookConfig::from_durable_snapshot_config(artifact)
        .expect("hydrate binary durable Hook matcher artifact");
    eprintln!(
        "binaryHookArtifactHydrateMicros={}",
        hydrate_started.elapsed().as_micros()
    );
    assert_eq!(recovered.rule_count(), live.rule_count());
}

#[test]
fn command_shard_covers_every_configured_profile_and_rule_pattern_without_literal_fixtures() {
    let live = ClientHookConfig::default();
    let shard = live
        .durable_command_profile_decision_shard()
        .expect("compile command-profile decision shard");
    let runtime = crate::HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: live.provider_projections.clone(),
    };
    let configured_prefixes = live
        .durable_command_decision_prefixes()
        .expect("derive all finite command prefixes");
    assert!(!configured_prefixes.is_empty());
    for prefix in configured_prefixes {
        let mut absolute_prefix = prefix.clone();
        absolute_prefix[0] = format!("/runtime/bin/{}", absolute_prefix[0]);
        let command = absolute_prefix.join(" ");
        let payload = serde_json::json!({
            "tool_name": "Bash",
            "session_id": "config-derived-command-shard-root",
            "tool_input": { "command": command },
        });
        let tokens = crate::semantic_shell_tokens(&command);
        let actual = crate::CommandDecisionShard::select(&shard, &tokens)
            .expect("decode command-profile shard")
            .expect("every configured prefix has a shard decision");
        let actual = crate::rebind_command_decision_to_payload(actual, &payload);
        let action = crate::tool_action::ToolAction::normalized_shell_command_action(
            command.clone(),
            "Bash".to_owned(),
        );
        let expected = live
            .classify_candidate(&runtime, "codex", "pre-tool", &action)
            .map(|candidate| candidate.decision)
            .unwrap_or_else(|| {
                crate::classifier::default_allow_for_normalized_action("codex", "pre-tool", &action)
            });
        let expected = crate::rebind_command_decision_to_payload(expected, &payload);
        assert_eq!(
            actual.decision, expected.decision,
            "prefix={prefix:?} actual={actual:?} expected={expected:?}"
        );
        assert_eq!(
            actual.reason_kind, expected.reason_kind,
            "prefix={prefix:?}"
        );
        assert_eq!(actual.message, expected.message, "prefix={prefix:?}");
        assert_eq!(
            actual.fields.get("configRuleId"),
            expected.fields.get("configRuleId"),
            "prefix={prefix:?}"
        );
        if expected.reason_kind == crate::ReasonKind::SubagentReceiptRequired {
            let target_role = actual
                .fields
                .get("targetAgentRole")
                .and_then(serde_json::Value::as_str)
                .expect("dispatch decision projects its ChoicePlane target role");
            assert!(!target_role.is_empty());
            assert_eq!(
                actual.fields.get("agentSessionAction"),
                Some(&serde_json::json!("dispatch-choice-plane-role"))
            );
            assert!(
                actual
                    .fields
                    .get("receiptKind")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.is_empty())
            );
            assert!(!actual.fields.contains_key("targetAgentName"));
        }
    }
}
