use std::collections::BTreeSet;

use crate::ClientHookConfig;

use super::validate_match_policy_rule_sets;

#[test]
fn missing_canonical_rule_is_rejected_before_matcher_publication() {
    let canonical = ClientHookConfig::default()
        .rule_ids()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut configured = canonical.clone();
    assert!(configured.remove("route-read-to-asp-languages"));

    let error = validate_match_policy_rule_sets(&configured, &canonical)
        .expect_err("missing canonical Read rule must fail closed");
    assert!(error.contains("route-read-to-asp-languages"), "{error}");
    assert!(error.contains("canonicalOnly"), "{error}");
}
