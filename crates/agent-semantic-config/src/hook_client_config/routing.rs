//! Declarative Hook routing rules and their typed dispatch projections.

use serde::Deserialize;
use serde::Serialize;

/// One declarative hook rule from project-local config.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleConfig {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i64,
    /// Stop Hook policy evaluation after this rule wins. Terminal rules are
    /// restricted to declarative `allow` decisions by validation.
    #[serde(default)]
    pub terminal: bool,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub fields: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub dispatch: Option<HookClientRuleDispatchConfig>,
    pub decision: HookClientConfigDecision,
    #[serde(default)]
    pub reason_kind: Option<HookClientConfigReasonKind>,
    #[serde(default)]
    pub message: Option<String>,
    /// Native Host matcher aliases admitted by this rule, joined with `|`.
    #[serde(default)]
    pub matcher: Option<String>,
    #[serde(default)]
    pub actions: Vec<HookClientActionKind>,
    #[serde(default)]
    pub profiles_list: Vec<String>,
    #[serde(default)]
    pub matcher_policies: Vec<HookClientMatcherPolicy>,
    #[serde(default)]
    pub language_ids: Vec<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default, rename = "match")]
    pub match_config: HookClientRuleMatchConfig,
    #[serde(default)]
    pub routes: Vec<HookClientRuleRouteConfig>,
}

/// Stable agent-registry route key carried directly by a dispatch rule.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HookClientAgentSelector(String);

impl HookClientAgentSelector {
    /// Returns the exact agent-registry route key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Typed receipt category emitted by a config-driven dispatch.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HookClientReceiptKind(String);

impl HookClientReceiptKind {
    /// Returns the exact receipt category spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Declarative dispatch selected after a Hook rule denies the current Agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleDispatchConfig {
    pub transport: HookClientRuleDispatchTransport,
    /// Exact route key from `agents/config.toml`. Host-specific names and
    /// invocation syntax are projected by the active Host adapter.
    pub agent: HookClientAgentSelector,
    pub receipt_kind: HookClientReceiptKind,
    #[serde(default)]
    pub lazy_provider: Option<HookClientLazyProviderPolicy>,
}

/// Supported host transport for config-driven execution dispatch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientRuleDispatchTransport {
    HostAgent,
}

/// Declarative provider materialization policy for a Host role dispatch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientLazyProviderPolicy {
    MatchedLanguage,
}

impl HookClientRuleDispatchTransport {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostAgent => "host-agent",
        }
    }
}

/// Shared host action spelling used by declarative hook rules.
/// Semantic action classified from an admitted Host invocation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[derive(Hash)]
pub enum HookClientActionKind {
    Read,
    Search,
    Edit,
    Enumerate,
    Execute,
    Mcp,
    SpawnAgent,
    Unknown,
}

/// Host invocation shape admitted by a declarative matcher.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[derive(Hash)]
pub enum HookClientHostInvocationKind {
    Read,
    Edit,
    Search,
    Execute,
    Mcp,
    SpawnAgent,
    Unknown,
}

/// Policy controlling how native Host matcher evidence is interpreted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookClientMatcherPolicy {
    WrappedCommand,
}

/// Subject class attached to a semantic Hook action.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientActionSubjectKind {
    RegisteredLanguageSource,
    RegisteredLanguageSourcePattern,
    ProviderConfigFile,
    Directory,
    StructuralSelector,
    Other,
}

/// Rule match axes from project-local hook config.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleMatchConfig {
    /// Reusable typed action predicates that must all match this rule.
    #[serde(default)]
    pub capability_policy_all: Vec<String>,
    /// Reusable typed action predicates of which at least one must match.
    #[serde(default)]
    pub capability_policy_any: Vec<String>,
    /// Reusable typed action predicates none of which may match.
    #[serde(default)]
    pub capability_policy_none: Vec<String>,
    #[serde(default)]
    pub command_profile_any: Vec<super::profiles::HookClientCommandProfileRef>,
    /// Repository-wide command families, independent of language profiles.
    #[serde(default)]
    pub command_set_any: Vec<String>,
    #[serde(default, skip_deserializing, skip_serializing)]
    pub native_matcher_any: Vec<String>,
    #[serde(default, skip_deserializing, skip_serializing)]
    pub host_invocation_any: Vec<HookClientHostInvocationKind>,
    #[serde(default)]
    pub subject_kind_any: Vec<HookClientActionSubjectKind>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub tool_any: Vec<String>,
    #[serde(default)]
    pub command_any: Vec<String>,
    #[serde(default)]
    pub argv_pattern_any: Vec<Vec<String>>,
    /// Exact argument-vector prefixes evaluated at each parsed shell-command stage.
    /// For example, `argvPrefixAny = [["rm", "-rf"]]` matches `rm -rf target`.
    #[serde(default)]
    pub argv_prefix_any: Vec<Vec<String>>,
    /// Exact argv tokens that must coexist in one parser-owned shell stage.
    #[serde(default)]
    pub argv_token_all: Vec<String>,
    /// Exact shell environment assignments in the leading assignment block of
    /// the first parsed command stage.
    #[serde(default)]
    pub process_environment_assignment_any: Vec<String>,
    #[serde(default)]
    pub command_contains_any: Vec<String>,
    #[serde(default)]
    pub path_any: Vec<String>,
    #[serde(default)]
    pub path_glob_any: Vec<String>,
    #[serde(default, skip_deserializing, skip_serializing)]
    pub profile_extension_any: Vec<String>,
    #[serde(default, skip_deserializing, skip_serializing)]
    pub profile_any: Vec<super::document::HookClientProfileConfig>,
    #[serde(default)]
    pub argv_source_any: Vec<String>,
    #[serde(default)]
    pub argv_source_glob_any: Vec<String>,
    #[serde(default)]
    pub argv_source_exclude_flag_any: Vec<String>,
    /// Match when a parsed command stage carries a regular file owned by the workspace.
    #[serde(default)]
    pub argv_workspace_regular_file: bool,
    /// Match when a parsed command stage carries a supported structured document
    /// owned by the workspace. The format set is the same typed set used by
    /// structured projector capabilities, rather than provider language sources.
    #[serde(default)]
    pub argv_structured_document_file: bool,
    /// Match source paths owned by any activated language harness coverage contract.
    #[serde(default)]
    pub argv_registered_source_file: bool,
    /// Complete parser-owned structured projection matcher and lazy capability declaration.
    #[serde(default)]
    pub structured_projection: Option<HookClientStructuredProjectionMatchConfig>,
}

/// Named, reusable predicate over the normalized AgentAction envelope.
///
/// Policies own matching facts only. Decisions, priorities, messages, and
/// dispatch remain rule-owned so composing predicates cannot accidentally
/// compose side effects.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientCapabilityPolicyConfig {
    pub id: String,
    #[serde(default)]
    pub host_invocation_any: Vec<HookClientHostInvocationKind>,
    #[serde(default)]
    pub semantic_capability_any: Vec<HookClientActionKind>,
    #[serde(default)]
    pub subject_kind_any: Vec<HookClientActionSubjectKind>,
}

/// Structured document formats understood by Hook projector capabilities.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientStructuredFormat {
    Json,
    Toml,
}

/// Bounded structured-document projection recognized by a Hook rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientStructuredProjectionMatchConfig {
    pub binary: String,
    pub document_format: HookClientStructuredFormat,
    pub filter_grammar: HookClientStructuredFilterGrammar,
    /// Maximum number of array entries one finite slice may project.
    pub max_slice_items: usize,
    #[serde(default)]
    pub optional_subcommand_any: Vec<String>,
    #[serde(default)]
    pub option_any: Vec<String>,
    #[serde(default)]
    pub option_value_arity: std::collections::BTreeMap<String, u8>,
}

/// Filter grammar admitted for a structured-document projection.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientStructuredFilterGrammar {
    BoundedPathV1,
}

/// Route suggestion from project-local hook config.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleRouteConfig {
    pub provider_id: String,
    #[serde(default)]
    pub language_id: Option<String>,
    #[serde(default)]
    pub binary: Option<String>,
    pub kind: HookClientConfigRouteKind,
    pub argv: Vec<String>,
    #[serde(default)]
    pub stdin_mode: Option<HookClientConfigStdinMode>,
}

/// Config-level decision spelling for a rule.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigDecision {
    Allow,
    Block,
    Deny,
}

/// Config-level reason category spelling for a rule.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigReasonKind {
    None,
    RegisteredSourceRouteRequired,
    StructuredSourceRead,
    BulkSourceDump,
    RawBroadSearch,
    AgentSearchJson,
    AgentChoiceRequired,
}

/// Config-level route kind spelling for a rule route.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigRouteKind {
    Playbook,
    Query,
    Read,
    CheckChanged,
}

/// Config-level stdin handling spelling for a route.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigStdinMode {
    None,
    PipeCandidates,
    PipeDiff,
    Unknown,
}

fn default_enabled() -> bool {
    true
}
