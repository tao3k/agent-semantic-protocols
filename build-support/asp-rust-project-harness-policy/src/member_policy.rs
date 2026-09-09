// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Central Rust harness policy registry for ASP workspace member crates.

/// A source owner covered by a member crate harness policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessOwnerPolicy {
    pub path: &'static str,
    pub rationale: &'static str,
}

/// One centralized diagnostic severity override for a workspace member.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AspRustProjectHarnessDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// One centralized diagnostic severity override for a workspace member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessSeverityPolicy {
    pub rule_code: &'static str,
    pub severity: AspRustProjectHarnessDiagnosticSeverity,
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

/// Direct normal dependencies forbidden by a member's owner boundary.
#[must_use]
pub fn asp_workspace_member_forbidden_normal_dependencies(
    package_name: &str,
) -> &'static [&'static str] {
    match package_name {
        "agent-semantic-artifacts" => &["turso"],
        "agent-semantic-client-core" | "agent-semantic-runtime-server" => &["agent-semantic-hook"],
        _ => &[],
    }
}

impl AspRustProjectHarnessMemberPolicy {
    /// Return the content-addressed identity of the complete member policy.
    #[must_use]
    pub fn contract_digest(self) -> String {
        let severity_overrides = self
            .rule_severity_overrides
            .iter()
            .map(|policy| {
                serde_json::json!({
                    "ruleCode": policy.rule_code,
                    "severity": policy.severity,
                })
            })
            .collect::<Vec<_>>();
        let owner_projection = |owners: &[AspRustProjectHarnessOwnerPolicy]| {
            owners
                .iter()
                .map(|owner| {
                    serde_json::json!({
                        "path": owner.path,
                        "rationale": owner.rationale,
                    })
                })
                .collect::<Vec<_>>()
        };
        let material = serde_json::json!({
            "schemaId": "agent.semantic-protocols.rust-harness-member-policy",
            "schemaVersion": "1",
            "packageName": self.package_name,
            "crateRoot": self.crate_root,
            "cargoCheckAdviceAllowExplanation": self.cargo_check_advice_allow_explanation,
            "verificationLabel": self.verification_label,
            "severityOverrides": severity_overrides,
            "criterionPerformanceVerification": self.criterion_performance_verification,
            "latencySensitivePerformanceOwners": owner_projection(
                self.latency_sensitive_performance_owners,
            ),
            "availabilityStabilityOwners": owner_projection(self.availability_stability_owners),
        });
        let encoded = serde_json::to_vec(&material).expect("serialize static Rust harness policy");
        format!("blake3-256:{}", blake3::hash(&encoded).to_hex())
    }

    /// Builds the explicit ASP Rust execution config for this package.
    ///
    /// Cargo compiles the shared scanner once in the host dependency graph;
    /// each package build script applies this config to its own crate root.
    #[cfg(feature = "workspace-policy")]
    pub fn apply_to_asp_rust_config(
        self,
        config: asp_rust::AspRustConfig,
    ) -> asp_rust::AspRustConfig {
        let mut config = config
            .with_cargo_check_advice_allow_explanation(self.cargo_check_advice_allow_explanation);
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
            let severity = match severity_override.severity {
                AspRustProjectHarnessDiagnosticSeverity::Info => {
                    asp_rust::RustDiagnosticSeverity::Info
                }
                AspRustProjectHarnessDiagnosticSeverity::Warning => {
                    asp_rust::RustDiagnosticSeverity::Warning
                }
                AspRustProjectHarnessDiagnosticSeverity::Error => {
                    asp_rust::RustDiagnosticSeverity::Error
                }
            };
            config = config.with_rule_severity(severity_override.rule_code, severity);
        }
        config
    }
}

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

const RUNTIME_SERVER_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/query_generation.rs",
        rationale: "Runtime generation admission, bounded calibration, and resident attachment publication control cold-to-warm Search latency",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/runtime_asp_client.rs",
        rationale: "Runtime ClientFrame dispatch and terminalization sit on every public Search and Query request",
    },
];

const RUNTIME_SERVER_STABILITY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/query_generation.rs",
        rationale: "ProjectId and WorkspaceId generation partitions must preserve single-flight, drain, and exact content binding",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/runtime_asp_client.rs",
        rationale: "Every admitted request must emit exactly one typed terminal without raw EOF or client fallback",
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
        package_name: "agent-semantic-hook-testkit",
        crate_root: "crates/agent-semantic-hook-testkit",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-hook-testkit cargo-check advice; owner=agent-semantic-hook-testkit build gate; finding_category=advisory policy findings; why_safe_now=the Hook TestKit keeps advisory findings visible while warning and error findings still fail the build; cleanup_trigger=clear the crate advisory backlog and remove this allowance",
        verification_label: Some("Hook TestKit"),
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-http-json",
        crate_root: "crates/agent-semantic-http-json",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-http-json cargo-check advice; owner=shared loopback HTTP JSON transport build gate; finding_category=advisory policy findings; why_safe_now=the crate now keeps its facade thin while runtime-ownership advice remains visible and warning or error findings fail the build; cleanup_trigger=move connection tasks under the shared runtime owner and remove this allowance",
        verification_label: Some("HTTP JSON transport"),
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: &[],
        availability_stability_owners: &[],
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-provider-protocol",
        crate_root: "crates/agent-semantic-provider-protocol",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-provider-protocol cargo-check advice; owner=provider wire-contract build gate; finding_category=advisory policy findings; why_safe_now=provider protocol keeps schema and typed transport advice visible while warning and error findings fail the build; cleanup_trigger=clear the provider contract advisory backlog and remove this allowance",
        verification_label: Some("provider protocol"),
        rule_severity_overrides: &[],
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
    AspRustProjectHarnessMemberPolicy {
        package_name: "agent-semantic-runtime-server",
        crate_root: "crates/agent-semantic-runtime-server",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-runtime-server cargo-check advice; owner=Runtime Server generation and ClientFrame build gate; finding_category=advisory policy findings; why_safe_now=advisory findings remain visible while warning and error findings fail the explicit workspace policy gate; cleanup_trigger=clear Runtime Server owner-size and lifecycle findings before publication",
        verification_label: Some("Runtime Server"),
        rule_severity_overrides: &[],
        criterion_performance_verification: true,
        latency_sensitive_performance_owners: RUNTIME_SERVER_LATENCY_OWNERS,
        availability_stability_owners: RUNTIME_SERVER_STABILITY_OWNERS,
    },
    AspRustProjectHarnessMemberPolicy {
        package_name: "orgize",
        crate_root: "languages/orgize",
        cargo_check_advice_allow_explanation: "scope=orgize explicit verification; owner=orgize parser build gate; finding_category=advisory policy findings; why_safe_now=ordinary downstream builds consume only the content-addressed member policy while the explicit Orgize verification gate runs the full harness; cleanup_trigger=clear the parser advisory backlog before tightening the explicit verification profile",
        verification_label: Some("Orgize parser"),
        rule_severity_overrides: &[AspRustProjectHarnessSeverityPolicy {
            rule_code: "RUST-MOD-R002",
            severity: AspRustProjectHarnessDiagnosticSeverity::Info,
        }],
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
