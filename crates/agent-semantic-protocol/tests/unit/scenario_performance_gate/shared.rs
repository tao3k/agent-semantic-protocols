use std::collections::BTreeMap;

use serde::Deserialize;

pub(super) const AGENT_POLICY_ID_GRAMMAR: &str = "<LANGUAGE>-AGENT-<TAGS>-<NUMBER>";
pub(super) const LARGE_LIBRARY_STEP_MAX_ELAPSED_MS: u64 = 300;
pub(super) const JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS: u64 = 5_000;
pub(super) const JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS: u64 = 75;
pub(super) const JULIA_DATAFRAMES_BATCH_SAMPLE_COUNT: usize = 3;
pub(super) const REQUIRED_PERFORMANCE_SENSITIVE_SUBCOMMAND_POLICY_IDS: &[&str] = &[
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-AGENT-SESSION-STATUS-REUSE-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-QUERY-SELECTOR-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-QUERY-TREESITTER-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-DEPS-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FD-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FD-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FZF-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FZF-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-OWNER-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-SOURCE-INDEX-MISSING-DB-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-SOURCE-INDEX-HIT-COLD-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-DYNAMIC-OVERLAY-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-SELECTOR-SEED-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-SOURCE-INDEX-MISS-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-PROVIDER-FACTS-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SOURCE-INDEX-LOOKUP-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-GRAPH-ROUTE-EVIDENCE-RANK-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-QUERY-SELECTOR-DIRECTORY-CODE-PREFLIGHT-COLD-001",
    "GERBIL-SCHEME-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "JULIA-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "ORG-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "PYTHON-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "PYTHON-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-OWNER-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-PROVIDER-CANDIDATE-ANNOTATIONS-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-CANDIDATE-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-EVIDENCE-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-NODE-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-OWNER-RANK-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-QUERY-OWNER-SEED-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-SEED-DECISION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-TOPOLOGY-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-PACKAGE-COHESION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-QUERY-PACK-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-QUALITY-DECISION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-EVIDENCE-CLASSIFIER-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-GENERATED-CANDIDATE-COLD-001",
    "TYPESCRIPT-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "TYPESCRIPT-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-OWNER-001",
];
pub(super) const REQUIRED_WORKSPACE_ARGUMENT_POLICY_IDS: &[&str] =
    &["RUST-AGENT-ASP-WORKSPACE-FILE-001"];
pub(super) const SHARED_SCENARIO_BENCHMARK_SCHEMA: &str =
    "schemas/semantic-scenario-benchmark.v1.schema.json";
pub(super) const SHARED_AGENT_POLICY_ID_SCHEMA: &str =
    "schemas/semantic-agent-policy-id.v1.schema.json";
pub(super) const LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS:
    &[LanguageScenarioBenchmarkRequirement] = &[
    LanguageScenarioBenchmarkRequirement {
        language: "rust",
        root: "languages/rust-lang-project-harness/tests/unit/scenarios",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "typescript",
        root: "languages/typescript-lang-project-harness/tests/unit/scenarios/software_criteria",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "python",
        root: "languages/python-lang-project-harness/tests/unit/harness/scenarios/software_criteria",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "julia",
        root: "languages/JuliaLangProjectHarness.jl/test/unit/scenarios/software_criteria",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "gerbil-scheme",
        root: "languages/gerbil-scheme-language-project-harness/t/scenarios/policy",
        syntax: ScenarioBenchmarkSyntax::GerbilBenchmarkSs,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "orgize",
        root: "languages/orgize/tests/unit/scenarios",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
];

pub(super) const COLD_FIRST_SEARCH_LANGUAGE_IDS: &[&str] =
    &["rust", "typescript", "python", "julia", "orgize"];

#[derive(Clone, Copy, Debug)]
pub(super) struct LanguageScenarioBenchmarkRequirement {
    pub(super) language: &'static str,
    pub(super) root: &'static str,
    pub(super) syntax: ScenarioBenchmarkSyntax,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ScenarioBenchmarkSyntax {
    TomlPair,
    GerbilBenchmarkSs,
}

#[derive(Debug, Deserialize)]
pub(super) struct ScenarioPolicyIds {
    #[serde(default)]
    pub(super) policy_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SharedScenarioToml {
    pub(super) id: String,
    pub(super) title: String,
    #[serde(default)]
    pub(super) policy_ids: Vec<String>,
    pub(super) agent_goal: String,
    pub(super) inputs: String,
    pub(super) expected: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SharedBenchmarkToml {
    pub(super) harness: String,
    #[serde(default)]
    pub(super) test: Option<String>,
    #[serde(default)]
    pub(super) bench: Option<String>,
    #[serde(default)]
    pub(super) phase: Option<String>,
    pub(super) target_total: String,
    pub(super) max_total: String,
    pub(super) observed_total: String,
    pub(super) regression_budget: String,
    pub(super) memory_budget_bytes: u64,
    pub(super) observed_memory_bytes: u64,
    pub(super) target_rationale: String,
    pub(super) observed_timings: BTreeMap<String, toml::Value>,
    #[serde(default)]
    pub(super) route_source: Option<String>,
    #[serde(default)]
    pub(super) max_provider_process_count: Option<u32>,
    #[serde(default)]
    pub(super) max_stdout_bytes: Option<u64>,
    #[serde(default)]
    pub(super) fallback_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchFrameReferenceBenchmarkToml {
    #[serde(default, rename = "codebaseMemoryMcpQueryRounds")]
    pub(super) codebase_memory_mcp_query_rounds: Option<u32>,
    #[serde(default, rename = "aspSearchQueryRounds")]
    pub(super) asp_search_query_rounds: Option<u32>,
    #[serde(default, rename = "roundDelta")]
    pub(super) round_delta: Option<i32>,
    #[serde(default, rename = "selectorEvidence")]
    pub(super) selector_evidence: Option<String>,
    #[serde(default, rename = "evidenceAdvantage")]
    pub(super) evidence_advantage: Option<String>,
}
