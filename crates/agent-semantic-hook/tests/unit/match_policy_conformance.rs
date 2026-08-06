use std::collections::BTreeSet;

use super::{production_cases, validate_match_policy_rule_sets};

#[test]
fn missing_canonical_rule_is_rejected_before_matcher_publication() {
    let witnessed = production_cases()
        .into_iter()
        .map(|case| case.rule_id.to_owned())
        .collect::<BTreeSet<_>>();
    let mut configured = witnessed.clone();
    assert!(configured.remove("materialize-registered-source-read-action"));

    let error = validate_match_policy_rule_sets(&configured, &witnessed)
        .expect_err("missing canonical Read rule must fail closed");
    assert!(
        error.contains("materialize-registered-source-read-action"),
        "{error}"
    );
    assert!(error.contains("witnessOnly"), "{error}");
}
