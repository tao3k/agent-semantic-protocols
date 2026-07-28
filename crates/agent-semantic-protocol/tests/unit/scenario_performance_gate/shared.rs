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
    "GERBIL-SCHEME-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-DEPS-STDLIB-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FD-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FD-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-LEXICAL-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-LEXICAL-SOURCE-INDEX-001",
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

pub(super) struct OwnerItemsProviderFixture<'a> {
    pub(super) binary_path: &'a std::path::Path,
    pub(super) count_path: &'a std::path::Path,
    pub(super) language_id: &'a str,
    pub(super) owner_path: &'a str,
    pub(super) query: &'a str,
    pub(super) item_symbol: &'a str,
    pub(super) algorithm: &'a str,
    pub(super) source_byte_end: usize,
}

pub(super) fn owner_items_fixture_uses_native_transport(language_id: &str) -> bool {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .unwrap_or_else(|| panic!("missing provider manifest for {language_id}"));
    agent_semantic_hook::registered_provider_method_invocation_v1(
        language_id,
        manifest.provider_id().as_str(),
        "search/owner-native-v1",
    )
    .expect("resolve fixture owner transport")
    .is_some()
}

pub(super) fn write_owner_items_provider_fixture(spec: OwnerItemsProviderFixture<'_>) {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == spec.language_id)
        .unwrap_or_else(|| panic!("missing provider manifest for {}", spec.language_id));
    let native_owner = owner_items_fixture_uses_native_transport(spec.language_id);
    let native_owner_flag = if native_owner { "1" } else { "0" };
    let legacy_stdout = format!(
        "[search-owner] q={} pkg=. selector=items alg={}\nO=owner:path({})!owner;I=item:symbol({})@{}:1:1!syntax\n",
        spec.owner_path, spec.algorithm, spec.owner_path, spec.item_symbol, spec.owner_path,
    );
    let script = format!(
        "#!/bin/sh\n\
count=0\n\
if [ -f {count} ]; then count=$(cat {count}); fi\n\
count=$((count + 1))\n\
printf '%s' \"$count\" > {count}\n\
native_owner={native_owner_flag}\n\
transport_owner=0\n\
for arg in \"$@\"; do\n\
  if [ \"$arg\" = 'owner-search-stdin' ]; then transport_owner=1; fi\n\
done\n\
if [ \"$native_owner\" = '1' ] && [ \"$transport_owner\" = '1' ]; then\n\
  request=$(cat)\n\
  digest=$(printf '%s' \"$request\" | sed -n 's/.*\"contentDigest\":\"\\([0-9a-f]*\\)\".*/\\1/p')\n\
  printf '%s\\n' \"{{\\\"schemaId\\\":\\\"agent.semantic-protocols.provider-native-owner-search-response\\\",\\\"schemaVersion\\\":\\\"1\\\",\\\"languageId\\\":\\\"{language_id}\\\",\\\"providerId\\\":\\\"{provider_id}\\\",\\\"requestedOwnerPath\\\":\\\"{owner_path}\\\",\\\"requestedQuery\\\":\\\"{query}\\\",\\\"sourceContentDigest\\\":\\\"$digest\\\",\\\"parsedOwnerCount\\\":1,\\\"projectionCompleteness\\\":\\\"complete-owner\\\",\\\"projections\\\":[{{\\\"structuralSelector\\\":\\\"{language_id}://{owner_path}#item/function/{item_symbol}\\\",\\\"itemKind\\\":\\\"function\\\",\\\"itemName\\\":\\\"{item_symbol}\\\",\\\"captureName\\\":\\\"function.name\\\",\\\"signature\\\":\\\"fn {item_symbol}\\\",\\\"sourceByteStart\\\":0,\\\"sourceByteEnd\\\":{source_byte_end}}}]}}\"\n\
  exit 0\n\
fi\n\
printf '%s' {legacy_stdout}\n",
        count = shell_single_quote_fixture(spec.count_path.to_string_lossy().as_ref()),
        native_owner_flag = native_owner_flag,
        language_id = spec.language_id,
        provider_id = manifest.provider_id(),
        owner_path = spec.owner_path,
        query = spec.query,
        item_symbol = spec.item_symbol,
        source_byte_end = spec.source_byte_end,
        legacy_stdout = shell_single_quote_fixture(&legacy_stdout),
    );
    std::fs::write(spec.binary_path, script).expect("write owner-items provider fixture");
    let mut permissions = std::fs::metadata(spec.binary_path)
        .expect("owner-items provider fixture metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(spec.binary_path, permissions)
        .expect("chmod owner-items provider fixture");
}

fn shell_single_quote_fixture(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
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
