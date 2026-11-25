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
        path: "src/converged_resident_index/query.rs",
        rationale: "resident multi-axis query execution sits on every warm search hot path",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/trigram_store/store.rs",
        rationale: "compact 6-byte trigram posting intersection governs resident candidate pruning latency",
    },
    AspRustProjectHarnessOwnerPolicy {
        path: "src/source_index_rank.rs",
        rationale: "deterministic candidate ranking must remain below the warm search latency gate",
    },
];

const SEARCH_PROJECTION_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/resident_search_result.rs",
        rationale: "language-neutral resident result construction and zero-I/O proof are on the warm search boundary",
    },
];

const TOPOLOGY_INDEX_LATENCY_OWNERS: &[AspRustProjectHarnessOwnerPolicy] = &[
    AspRustProjectHarnessOwnerPolicy {
        path: "src/topology_index.rs",
        rationale: "rarest-first cross-language topology feature intersection is a resident Search hot path",
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
        package_name: "agent-semantic-topology",
        crate_root: "crates/agent-semantic-topology",
        cargo_check_advice_allow_explanation: "scope=agent-semantic-topology cargo-check advice; owner=language-neutral project topology and resident index gate; finding_category=advisory policy findings; why_safe_now=the topology authority keeps advisory findings visible while warning and error findings fail the build; cleanup_trigger=clear the topology advisory backlog and remove this allowance",
        verification_label: Some("project topology index"),
        rule_severity_overrides: &[],
        criterion_performance_verification: false,
        latency_sensitive_performance_owners: TOPOLOGY_INDEX_LATENCY_OWNERS,
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
