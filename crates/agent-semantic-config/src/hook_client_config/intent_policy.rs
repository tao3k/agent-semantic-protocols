use serde::Deserialize;

/// Parser-owned policy for classifying public `asp <language>` commands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspCommandIntentPolicyConfig {
    #[serde(default)]
    pub control_plane: HookClientAspControlPlaneIntentConfig,
    #[serde(default)]
    pub reasoning: HookClientAspReasoningIntentConfig,
    #[serde(default)]
    pub exact_evidence: HookClientAspExactEvidenceIntentConfig,
    #[serde(default)]
    pub invalid_evidence: HookClientAspInvalidEvidenceIntentConfig,
}

impl Default for HookClientAspCommandIntentPolicyConfig {
    fn default() -> Self {
        Self {
            control_plane: HookClientAspControlPlaneIntentConfig::default(),
            reasoning: HookClientAspReasoningIntentConfig::default(),
            exact_evidence: HookClientAspExactEvidenceIntentConfig::default(),
            invalid_evidence: HookClientAspInvalidEvidenceIntentConfig::default(),
        }
    }
}

/// Root ASP commands that are operational control surfaces, not reasoning.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspControlPlaneIntentConfig {
    #[serde(default = "default_asp_control_plane_root_commands")]
    pub root_commands: Vec<String>,
}

impl Default for HookClientAspControlPlaneIntentConfig {
    fn default() -> Self {
        Self {
            root_commands: default_asp_control_plane_root_commands(),
        }
    }
}

/// Commands and routes that remain in the semantic reasoning lane.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspReasoningIntentConfig {
    #[serde(default = "default_asp_reasoning_root_commands")]
    pub root_commands: Vec<String>,
    #[serde(default = "default_asp_policy_true")]
    pub guide_command: bool,
    #[serde(default = "default_asp_reasoning_search_routes")]
    pub search_routes: Vec<String>,
    #[serde(default = "default_asp_reasoning_query_flags")]
    pub query_flags: Vec<String>,
    #[serde(default = "default_asp_policy_true")]
    pub unprojected_query: bool,
}

impl Default for HookClientAspReasoningIntentConfig {
    fn default() -> Self {
        Self {
            root_commands: default_asp_reasoning_root_commands(),
            guide_command: true,
            search_routes: default_asp_reasoning_search_routes(),
            query_flags: default_asp_reasoning_query_flags(),
            unprojected_query: true,
        }
    }
}

/// Projection and selector requirements for exact evidence reads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspExactEvidenceIntentConfig {
    #[serde(default = "default_asp_exact_evidence_projection_flags")]
    pub query_projection_flags: Vec<String>,
    #[serde(default = "default_asp_exact_evidence_projection_views")]
    pub query_projection_views: Vec<String>,
    #[serde(default = "default_asp_exact_selector_kinds")]
    pub selector_kinds: Vec<String>,
    #[serde(default = "default_asp_policy_true")]
    pub require_same_language: bool,
}

impl Default for HookClientAspExactEvidenceIntentConfig {
    fn default() -> Self {
        Self {
            query_projection_flags: default_asp_exact_evidence_projection_flags(),
            query_projection_views: default_asp_exact_evidence_projection_views(),
            selector_kinds: default_asp_exact_selector_kinds(),
            require_same_language: true,
        }
    }
}

/// Invalid evidence shapes that must be rejected rather than treated as reasoning.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspInvalidEvidenceIntentConfig {
    #[serde(default = "default_asp_policy_true")]
    pub reject_projected_query_without_exact_selector: bool,
    #[serde(default = "default_asp_policy_true")]
    pub reject_cross_language_selector: bool,
}

impl Default for HookClientAspInvalidEvidenceIntentConfig {
    fn default() -> Self {
        Self {
            reject_projected_query_without_exact_selector: true,
            reject_cross_language_selector: true,
        }
    }
}

fn default_asp_policy_true() -> bool {
    true
}

fn default_asp_control_plane_root_commands() -> Vec<String> {
    [
        "guide",
        "providers",
        "tools",
        "wrap",
        "cache",
        "cloud",
        "hook",
        "agent",
        "install",
        "sync",
        "paths",
        "healthcheck",
        "source-access",
        "ast-patch",
        "graph",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_asp_reasoning_search_routes() -> Vec<String> {
    [
        "prime",
        "pipe",
        "owner",
        "lexical",
        "deps",
        "dependency",
        "failure",
        "reasoning",
        "ingest",
        "guide",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_asp_reasoning_root_commands() -> Vec<String> {
    ["fd", "rg"].into_iter().map(str::to_string).collect()
}

fn default_asp_reasoning_query_flags() -> Vec<String> {
    ["--term"].into_iter().map(str::to_string).collect()
}

fn default_asp_exact_evidence_projection_flags() -> Vec<String> {
    ["--code", "--content", "--verbatim", "--names-only"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn default_asp_exact_evidence_projection_views() -> Vec<String> {
    ["metadata"].into_iter().map(str::to_string).collect()
}

fn default_asp_exact_selector_kinds() -> Vec<String> {
    ["item"].into_iter().map(str::to_string).collect()
}
