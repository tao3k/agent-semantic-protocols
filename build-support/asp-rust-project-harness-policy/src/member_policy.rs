//! Central Rust harness policy registry for ASP workspace member crates.

/// A source owner covered by a member crate harness policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessOwnerPolicy {
    pub path: &'static str,
    pub rationale: &'static str,
}

/// One centralized diagnostic severity override for a workspace member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessSeverityPolicy {
    pub rule_code: &'static str,
    pub severity: rust_lang_project_harness::RustDiagnosticSeverity,
}

/// Declarative Rust harness policy for one ASP workspace member crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessMemberPolicy {
    pub package_name: &'static str,
    pub crate_root: &'static str,
    pub cargo_check_advice_allow_explanation: &'static str,
    pub verification_label: Option<&'static str>,
    pub rule_severity_overrides: &'static [AspRustProjectHarnessSeverityPolicy],
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
        for severity_override in self.rule_severity_overrides {
            config =
                config.with_rule_severity(severity_override.rule_code, severity_override.severity);
        }
        config
    }
}

const PROTOCOL_SEVERITY_OVERRIDES: &[AspRustProjectHarnessSeverityPolicy] =
    &[AspRustProjectHarnessSeverityPolicy {
        rule_code: "RUST-AGENT-PROJECT-005",
        severity: rust_lang_project_harness::RustDiagnosticSeverity::Info,
    }];

const PROVIDER_TRANSPORT_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/capture.rs",
        rationale: "provider stdout/stderr capture is a hot path for every native provider command",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/transport.rs",
        rationale: "provider process orchestration controls command latency and timeout behavior",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/byte_text.rs",
        rationale: "byte-text projection is reused by compact search and provider output rendering",
    },
];

const PROVIDER_TRANSPORT_STABILITY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/transport.rs",
        rationale: "provider process orchestration must keep timeout, kill, and broken-pipe behavior deterministic",
    },
];

const SEARCH_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/resident_source_index.rs",
        rationale: "resident shallow and owner-local search lookup sits on every warm search hot path",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/graph_owner_rank.rs",
        rationale: "owner candidate ranking must remain below the warm search latency gate",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/search_generation_segment.rs",
        rationale: "immutable generation segment validation gates resident search admission",
    },
];

const SEARCH_PROJECTION_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/resident_search_result.rs",
        rationale: "language-neutral resident result construction and zero-I/O proof are on the warm search boundary",
    },
];

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

const CLIENT_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/cli.rs",
        rationale: "the thin client owns Runtime Server route submission and provider-register snapshot latency",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/search_history",
        rationale: "search history audit uses Turso-backed artifact timelines and graph-turbo dispatch",
    },
];

const CLIENT_STABILITY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/cli.rs",
        rationale: "the thin client must fail closed when Runtime Server route authority is unavailable",
    },
];

const ASP_WORKSPACE_MEMBER_POLICIES: &[AspRustProjectHarnessMemberPolicy] = &[
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-artifacts",
        crate_root: "crates/agent-semantic-artifacts",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-artifacts cargo-check advice; owner=artifact identity build gate; finding_category=advisory policy findings; why_safe_now=Phase 1 exposes only typed Merkle identity primitives and keeps DB/search side effects out of this crate while warning and error findings still fail the build; cleanup_trigger=clear any remaining advisory backlog before connecting artifact roots to DB writes",
        verification_label: None,
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client-core",
        crate_root: "crates/agent-semantic-client-core",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client-core cargo-check advice; owner=agent-semantic-client-core build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client-core keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client-db",
        crate_root: "crates/agent-semantic-client-db",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client-db cargo-check advice; owner=agent-semantic-client-db build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client-db keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("client db"),
        rule_severity_overrides: &[],
        criterion_performance_verification: true,
        latency_sensitive_performance_owners: CLIENT_DB_LATENCY_OWNERS,
        availability_stability_owners: CLIENT_DB_STABILITY_OWNERS,
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client-server",
        crate_root: "crates/agent-semantic-client-server",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client-server cargo-check advice; owner=agent-semantic-client-server build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client-server keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("ASP client server"),
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-client",
        crate_root: "crates/agent-semantic-client",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-client cargo-check advice; owner=agent-semantic-client build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-client keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("client"),
        rule_severity_overrides: &[],
        criterion_performance_verification: true,
        latency_sensitive_performance_owners: CLIENT_LATENCY_OWNERS,
        availability_stability_owners: CLIENT_STABILITY_OWNERS,
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-hook",
        crate_root: "crates/agent-semantic-hook",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-hook cargo-check advice; owner=agent-semantic-hook build gate; finding_category=advisory policy findings; why_safe_now=semantic-agent-hook keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-protocol",
        crate_root: "crates/agent-semantic-protocol",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-protocol cargo-check advice; owner=agent-semantic-protocol build gate; finding_category=advisory policy findings; why_safe_now=agent-semantic-protocol keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        rule_severity_overrides: PROTOCOL_SEVERITY_OVERRIDES,
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-provider-transport",
        crate_root: "crates/agent-semantic-provider-transport",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-provider-transport cargo-check advice; owner=agent-semantic-provider-transport build gate; finding_category=advisory policy findings; why_safe_now=provider transport keeps process orchestration advice visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("provider transport"),
        rule_severity_overrides: &[],
        criterion_performance_verification: true,
        latency_sensitive_performance_owners: PROVIDER_TRANSPORT_LATENCY_OWNERS,
        availability_stability_owners: PROVIDER_TRANSPORT_STABILITY_OWNERS,
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-search",
        crate_root: "crates/agent-semantic-search",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-search cargo-check advice; owner=agent-semantic-search build gate; finding_category=advisory policy findings; why_safe_now=search crate keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: SEARCH_LATENCY_OWNERS,
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-search-projection",
        crate_root: "crates/agent-semantic-search-projection",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-search-projection cargo-check advice; owner=search projection build gate; finding_category=advisory policy findings; why_safe_now=the projection package keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("search projection"),
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: SEARCH_PROJECTION_LATENCY_OWNERS,
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-schema-manager",
        crate_root: "crates/agent-semantic-schema-manager",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-schema-manager cargo-check advice; owner=canonical schema distribution build gate; finding_category=advisory policy findings; why_safe_now=the schema manager keeps canonical closure resolution and content-addressed publication isolated while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("schema manager"),
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-tree-sitter",
        crate_root: "crates/agent-semantic-tree-sitter",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-tree-sitter cargo-check advice; owner=agent-semantic-tree-sitter build gate; finding_category=advisory policy findings; why_safe_now=tree-sitter catalog ABI advice stays visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-runtime",
        crate_root: "crates/agent-semantic-runtime",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-runtime cargo-check advice; owner=agent-semantic-runtime build gate; finding_category=advisory policy findings; why_safe_now=runtime state materialization keeps filesystem side effects in a focused crate while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: None,
        rule_severity_overrides: &[],
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
        .iter()
        .find(|policy| policy.package_name == package_name)
}
