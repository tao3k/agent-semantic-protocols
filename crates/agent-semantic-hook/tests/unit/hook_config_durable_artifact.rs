use super::ClientHookConfig;

#[test]
fn complete_durable_hook_artifact_recovery_hydrate_p99_is_bounded() {
    let live = ClientHookConfig::default();
    let bytes = serde_json::to_vec(&live.durable_snapshot_config())
        .expect("serialize complete durable Hook matcher artifact");
    assert!(
        bytes.len() < 4 * 1024 * 1024,
        "complete durable Hook matcher artifact must fit the Runtime Server seqlock memory; observed {} bytes",
        bytes.len()
    );
    let mut samples = (0..100)
        .map(|_| {
            let started = std::time::Instant::now();
            let artifact = serde_json::from_slice(&bytes)
                .expect("decode complete durable Hook matcher artifact");
            let config = ClientHookConfig::from_durable_snapshot_config(artifact)
                .expect("hydrate complete durable Hook matcher artifact");
            std::hint::black_box(config.rule_count());
            started.elapsed()
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[hook-durable-artifact-recovery] bytes={} rules={} admissions=100 p99Nanos={}",
        bytes.len(),
        live.rule_count(),
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(50),
        "one-time durable Hook generation recovery must remain below 50 ms p99, observed {p99:?}"
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
fn command_profile_shard_covers_every_configured_prefix_without_literal_fixtures() {
    let live = ClientHookConfig::default();
    let source = live
        .source_config
        .as_ref()
        .expect("default config retains publication source");
    let shard = live
        .durable_command_profile_decision_shard()
        .expect("compile command-profile decision shard");
    let runtime = crate::HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: live.provider_projections.clone(),
    };
    let configured_prefixes = source
        .command_profiles
        .iter()
        .flat_map(|profile| profile.categories.values())
        .flatten()
        .collect::<Vec<_>>();
    assert!(!configured_prefixes.is_empty());
    for prefix in configured_prefixes {
        let command = format!("direnv exec . /bin/bash -c {}", prefix.join(" "));
        let payload = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": { "command": command },
        });
        let tokens = crate::semantic_shell_tokens(&command);
        let actual = crate::CommandDecisionShard::select(&shard, &tokens)
            .expect("decode command-profile shard")
            .expect("every configured prefix has a shard decision");
        let actual = crate::rebind_command_decision_to_payload(actual, &payload);
        let expected = crate::classify_hook_with_config(crate::HookClassificationRequest {
            registry: &runtime,
            config: &live,
            platform: "codex",
            event: "pre-tool",
            payload: &payload,
        });
        assert_eq!(actual.decision, expected.decision, "prefix={prefix:?}");
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
            let typed_payload = serde_json::json!({
                "tool_name": "Bash",
                "agent_id": "asp_testing",
                "agent_type": "asp_testing",
                "is_subagent": true,
                "tool_input": { "command": command },
            });
            let raw = crate::CommandDecisionShard::select(&shard, &tokens)
                .expect("decode typed command-profile shard")
                .expect("typed command prefix has a shard decision");
            let typed = crate::rebind_command_decision_to_payload(raw, &typed_payload);
            assert_eq!(
                typed.decision,
                crate::DecisionKind::Allow,
                "prefix={prefix:?}"
            );
            assert_eq!(
                typed.fields.get("dispatchSatisfied"),
                Some(&serde_json::json!(true))
            );
        }
    }
}
