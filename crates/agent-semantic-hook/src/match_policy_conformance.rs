use crate::ClientHookConfig;
use std::collections::BTreeSet;

/// Reject an installed matcher whose managed rule identities drift from the
/// embedded production policy. Concrete payload witnesses belong to TestKit;
/// the runtime gate compares policy artifacts only.
pub fn validate_match_policy_rule_coverage(config: &ClientHookConfig) -> Result<(), String> {
    let configured = config
        .rule_ids()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let canonical = ClientHookConfig::default()
        .rule_ids()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    validate_match_policy_rule_sets(&configured, &canonical)
}

fn validate_match_policy_rule_sets(
    configured: &BTreeSet<String>,
    canonical: &BTreeSet<String>,
) -> Result<(), String> {
    if configured == canonical {
        return Ok(());
    }
    Err(format!(
        "configured and canonical rule IDs differ: configOnly={:?} canonicalOnly={:?}",
        configured.difference(canonical).collect::<Vec<_>>(),
        canonical.difference(configured).collect::<Vec<_>>()
    ))
}

#[cfg(test)]
#[path = "../tests/unit/match_policy_conformance.rs"]
mod tests;
