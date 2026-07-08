//! Central Rust harness policy registry for ASP workspace member crates.

/// A source owner covered by a member crate harness policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessOwnerPolicy {
    pub path: &'static str,
    pub rationale: &'static str,
}

/// Declarative Rust harness policy for one ASP workspace member crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessMemberPolicy {
    pub package_name: &'static str,
    pub crate_root: &'static str,
    pub cargo_check_advice_allow_explanation: &'static str,
    pub verification_label: Option<&'static str>,
    pub criterion_performance_verification: bool,
    pub latency_sensitive_performance_owners: &'static [AspRustProjectHarnessOwnerPolicy],
    pub availability_stability_owners: &'static [AspRustProjectHarnessOwnerPolicy],
}

impl AspRustProjectHarnessMemberPolicy {
    /// Builds the `rust-lang-project-harness` config for this member crate.
    pub fn to_harness_config(self) -> rust_lang_project_harness::RustHarnessConfig {
        let mut config = rust_lang_project_harness::RustHarnessConfig {
            cargo_check_advice_allow_explanation: Some(
                self.cargo_check_advice_allow_explanation.to_string(),
            ),
            ..Default::default()
        };
        if self.criterion_performance_verification {
            config = config.with_criterion_performance_verification();
        }
        for owner in self.latency_sensitive_performance_owners {
            config = config.with_latency_sensitive_performance_owner(owner.path, owner.rationale);
        }
        for owner in self.availability_stability_owners {
            config = config.with_availability_stability_owner(owner.path, owner.rationale);
        }
        config
    }
}

const CLIENT_DB_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/engine/facade.rs",
        rationale: "DB Engine facade routes provider replay hot paths through active Turso adapters",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/engine/turso_cache.rs",
        rationale: "Turso cache generation lookup and invalidation sit on repeated agent search replay paths",
    },
];

const CLIENT_DB_STABILITY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/engine/turso.rs",
        rationale: "Turso bootstrap schema and transaction boundaries must remain stable under repeated agent writeback and replay",
    },
];

const CLIENT_LOCAL_CLI_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/backend.rs",
        rationale: "local native backend fans out provider commands and aggregates captured Bytes output",
    },
];

const CLIENT_LOCAL_CLI_STABILITY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/backend.rs",
        rationale: "local native backend must preserve deterministic provider routing and error handling under repeated execution",
    },
];

const CLIENT_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/provider_method.rs",
        rationale: "provider method dispatch owns cache-hit, packet-first, and provider-exec latency",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/cache_replay/search_packet.rs",
        rationale: "search packet replay renders compact stdout on the cache hot path",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/search_history.rs",
        rationale: "search history audit uses Turso-backed artifact timelines and graph-turbo dispatch",
    },
];

const CLIENT_STABILITY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/provider_method.rs",
        rationale: "provider method dispatch must degrade predictably across cache miss, provider failure, and timeout paths",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/cache_replay/search_packet.rs",
        rationale: "search packet replay must keep stable output shape under repeated cache generations",
    },
];

const ASP_WORKSPACE_MEMBER_POLICIES: &[AspRustProjectHarnessMemberPolicy] = &[
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-artifacts",
        crate_root: "crates/agent-semantic-artifacts",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-artifacts cargo-check advice; owner=artifact identity build gate; finding_category=advisory policy findings; why_safe_now=Phase 1 exposes only typed Merkle identity primitives and keeps DB/search side effects out of this crate while warning and error findings still fail the build; cleanup_trigger=clear any remaining advisory backlog before connecting artifact roots to DB writes",
        verification_label: None,
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client-core",
        crate_root: "crates/agent-semantic-client-core",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client-core cargo-check advice; owner=agent-semantic-client-core build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client-core keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client-db",
        crate_root: "crates/agent-semantic-client-db",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client-db cargo-check advice; owner=agent-semantic-client-db build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client-db keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("client db"),
        criterion_performance_verification: true,
        latency_sensitive_performance_owners: CLIENT_DB_LATENCY_OWNERS,
        availability_stability_owners: CLIENT_DB_STABILITY_OWNERS,
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client-local-cli",
        crate_root: "crates/agent-semantic-client-local-cli",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client-local-cli cargo-check advice; owner=agent-semantic-client-local-cli build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client-local-cli keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("local native backend"),
        criterion_performance_verification: true,
        latency_sensitive_performance_owners: CLIENT_LOCAL_CLI_LATENCY_OWNERS,
        availability_stability_owners: CLIENT_LOCAL_CLI_STABILITY_OWNERS,
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client",
        crate_root: "crates/agent-semantic-client",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client cargo-check advice; owner=agent-semantic-client build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("client"),
        criterion_performance_verification: true,
        latency_sensitive_performance_owners: CLIENT_LATENCY_OWNERS,
        availability_stability_owners: CLIENT_STABILITY_OWNERS,
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-hook",
        crate_root: "crates/agent-semantic-hook",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-hook cargo-check advice; owner=agent-semantic-hook build gate; finding_category=advisory policy findings; why_safe_now=semantic-agent-hook keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
];

/// Returns the ASP workspace member policies centralized under this crate.
pub fn asp_workspace_member_policies() -> &'static [AspRustProjectHarnessMemberPolicy] {
    ASP_WORKSPACE_MEMBER_POLICIES
}
/// Returns the registered ASP Rust member policy for `package_name`.
pub fn asp_workspace_member_policy_for(
    package_name: &str,
) -> Option<&'static AspRustProjectHarnessMemberPolicy> {
    asp_workspace_member_policies()
        .into_iter()
        .find(|policy| policy.package_name == package_name)
}
