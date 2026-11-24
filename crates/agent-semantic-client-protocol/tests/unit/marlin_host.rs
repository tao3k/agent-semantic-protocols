// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use super::MARLIN_ADMITTED_METHODS;
use super::MARLIN_HOST_INTEGRATION_SCHEMA_ID;
use super::MARLIN_HOST_INTEGRATION_SCHEMA_VERSION;
use super::MARLIN_HOST_PLATFORM;
use super::MarlinAgentLineageId;
use super::MarlinCapabilityProfile;
use super::MarlinHostCorrelation;
use super::MarlinHostIntegrationBinding;
use super::MarlinHostIntegrationError;
use super::MarlinParentSessionId;

#[test]
fn marlin_host_binding_round_trips_and_validates_default_surface() {
    let correlation = MarlinHostCorrelation::new(
        "marlin-root-01",
        "marlin-session-02",
        Some(MarlinParentSessionId::new("marlin-root-01")),
        Some(MarlinAgentLineageId::new("marlin.root/researcher")),
    )
    .expect("valid Marlin correlation");
    let binding = MarlinHostIntegrationBinding::new(
        "marlin-host-local",
        "proj-marlin",
        "ws-main",
        "wt-main",
        MarlinCapabilityProfile::FullWorkspaceGeneration,
        Some(correlation),
        Some("/tmp/asp-state".to_owned()),
    )
    .expect("valid Marlin host binding");

    assert_eq!(binding.schema_id, MARLIN_HOST_INTEGRATION_SCHEMA_ID);
    assert_eq!(
        binding.schema_version,
        MARLIN_HOST_INTEGRATION_SCHEMA_VERSION
    );
    assert_eq!(binding.platform, MARLIN_HOST_PLATFORM);
    assert_eq!(
        binding.admitted_methods.len(),
        MARLIN_ADMITTED_METHODS.len()
    );

    let encoded = serde_json::to_value(&binding).expect("serialize Marlin host binding");
    let decoded: MarlinHostIntegrationBinding =
        serde_json::from_value(encoded).expect("deserialize Marlin host binding");
    assert_eq!(decoded, binding);
    decoded.validate().expect("decoded binding remains valid");
}

#[test]
fn marlin_host_binding_rejects_non_marlin_platform_and_retired_methods() {
    let mut binding = MarlinHostIntegrationBinding::new(
        "marlin-host-local",
        "proj-marlin",
        "ws-main",
        "wt-main",
        MarlinCapabilityProfile::ReadOnlySemantic,
        None,
        None,
    )
    .expect("valid base binding");

    binding.platform = "codex".to_owned();
    assert_eq!(
        binding.validate(),
        Err(MarlinHostIntegrationError::UnsupportedPlatform(
            "codex".to_owned()
        ))
    );

    binding.platform = MARLIN_HOST_PLATFORM.to_owned();
    binding.admitted_methods = vec!["asp.hook.memory.inbox.publish".to_owned()];
    assert_eq!(
        binding.validate(),
        Err(MarlinHostIntegrationError::UnadmittedMethod(
            "asp.hook.memory.inbox.publish".to_owned()
        ))
    );
}

#[test]
fn marlin_host_binding_reads_environment_and_rejects_parent_current_aliasing() {
    let env = BTreeMap::from([
        ("ASP_HOST_PLATFORM", "marlin"),
        ("MARLIN_HOST_ID", "marlin-host-1"),
        ("MARLIN_PROJECT_ID", "proj-1"),
        ("MARLIN_WORKSPACE_ID", "ws-1"),
        ("MARLIN_WORKTREE_ID", "wt-1"),
        ("MARLIN_ROOT_SESSION_ID", "root-1"),
        ("MARLIN_PARENT_SESSION_ID", "root-1"),
        ("MARLIN_SESSION_ID", "child-1"),
        ("MARLIN_AGENT_LINEAGE", "marlin.root/search"),
    ]);
    let bound =
        MarlinHostIntegrationBinding::from_environment(|key| env.get(key).map(|v| (*v).to_owned()))
            .expect("valid environment binding");
    assert_eq!(bound.host_id.as_str(), "marlin-host-1");
    assert_eq!(
        bound
            .correlation
            .as_ref()
            .expect("correlation")
            .session_id
            .as_str(),
        "child-1"
    );

    let aliased = MarlinHostCorrelation::new(
        "root-1",
        "same-session",
        Some(MarlinParentSessionId::new("same-session")),
        None,
    );
    assert_eq!(
        aliased,
        Err(MarlinHostIntegrationError::ParentSessionAliasesCurrent)
    );
}
