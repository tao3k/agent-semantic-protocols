use std::collections::BTreeSet;

use super::{
    HookPolicyCombinatorialStrategy, HookPolicyWitnessPolarity, combinatorial_policy_witnesses,
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
        .language_providers
        .iter()
        .flat_map(|provider| provider.source_extensions.iter().cloned())
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
            >= 6
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
