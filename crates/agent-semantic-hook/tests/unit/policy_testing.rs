use std::collections::BTreeSet;

use super::{
    HookPolicyCombinatorialStrategy, HookPolicyWitnessPolarity, combinatorial_policy_witnesses,
    combinatorial_positional_shell_witnesses,
};

#[test]
fn compiled_policy_axes_generate_balanced_complex_black_and_white_witnesses() {
    let config = toml::from_str(&crate::default_client_config_template())
        .expect("parse rendered default Hook config");
    let max_wrapper_depth = 4;
    let witnesses = combinatorial_policy_witnesses(
        &config,
        HookPolicyCombinatorialStrategy {
            max_wrapper_depth,
            include_negative_extension_mutation: true,
        },
    )
    .expect("generate combinatorial provider-extension policy witnesses");
    let configured_extensions = config
        .profiles
        .values()
        .flat_map(|profile| {
            profile
                .extension_any
                .iter()
                .map(|extension| format!(".{extension}"))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        witnesses
            .iter()
            .map(|witness| witness.source_extension.clone())
            .collect::<BTreeSet<_>>(),
        configured_extensions
    );
    assert_eq!(
        witnesses
            .iter()
            .filter(|witness| witness.polarity == HookPolicyWitnessPolarity::Black)
            .count(),
        witnesses
            .iter()
            .filter(|witness| witness.polarity == HookPolicyWitnessPolarity::White)
            .count()
    );
    assert_eq!(
        witnesses
            .iter()
            .map(|witness| witness.wrapper_depth)
            .collect::<BTreeSet<_>>()
            .len(),
        max_wrapper_depth + 1
    );
    assert!(
        witnesses
            .iter()
            .filter_map(|witness| witness.command_axis.as_ref())
            .collect::<BTreeSet<_>>()
            .len()
            >= 1
    );
    assert!(
        witnesses
            .iter()
            .map(|witness| witness.envelope_axis.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            >= 8
    );
    for white in witnesses
        .iter()
        .filter(|witness| witness.polarity == HookPolicyWitnessPolarity::White)
    {
        assert!(
            configured_extensions
                .iter()
                .all(|extension| !white.path.ends_with(extension)),
            "negative mutation remained provider-registered: {}",
            white.path
        );
    }
}

#[test]
fn positional_shell_witnesses_preserve_argv_structure_without_inventing_policy() {
    let config = toml::from_str(&crate::default_client_config_template())
        .expect("parse rendered default Hook config");
    let witnesses = combinatorial_positional_shell_witnesses(
        &config,
        HookPolicyCombinatorialStrategy {
            max_wrapper_depth: 3,
            include_negative_extension_mutation: true,
        },
    )
    .expect("generate positional shell witnesses");
    let covered_languages = witnesses
        .iter()
        .map(|witness| witness.language_id.as_str())
        .collect::<BTreeSet<_>>();
    let configured_languages = config
        .profiles
        .values()
        .map(|profile| profile.language_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(covered_languages, configured_languages);
    for witness in witnesses {
        let payload = serde_json::json!({
            "tool_name": witness.tool_name,
            "tool_input": witness.tool_input,
        });
        let candidates = crate::shell_read_source_keys(&payload);
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.path == witness.path),
            "registered argv sibling was not normalized: {}",
            witness.id
        );
        assert!(
            candidates.iter().any(|candidate| {
                candidate.path != witness.path && candidate.extension != witness.source_extension
            }),
            "positional witness omitted its unregistered argv sibling: {}",
            witness.id
        );
    }
}
