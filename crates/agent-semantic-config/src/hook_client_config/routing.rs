use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientDecisionMaterializer {
    AgentSearchJson,
    ApplyPatch,
    SourceAccess,
}

/// One declarative hook rule from project-local config.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleConfig {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub fields: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub dispatch: Option<HookClientRuleDispatchConfig>,
    pub decision: HookClientConfigDecision,
    #[serde(default)]
    pub decision_materializer: Option<HookClientDecisionMaterializer>,
    #[serde(default)]
    pub reason_kind: Option<HookClientConfigReasonKind>,
    #[serde(default)]
    pub message: Option<String>,
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

/// Typed execution dispatch emitted by a matched hook rule.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleDispatchConfig {
    pub transport: HookClientRuleDispatchTransport,
    pub resident_name: String,
    pub receipt_kind: String,
    #[serde(default)]
    pub lazy_provider: Option<HookClientLazyProviderPolicy>,
}

/// Supported host transport for config-driven execution dispatch.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientRuleDispatchTransport {
    ResidentAgent,
}

/// Declarative provider materialization policy for a resident dispatch.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientLazyProviderPolicy {
    MatchedLanguage,
}

impl HookClientRuleDispatchTransport {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResidentAgent => "resident-agent",
        }
    }
}

/// Shared host action spelling used by declarative hook rules.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientActionKind {
    Read,
    Edit,
    Search,
    Enumerate,
    Execute,
    Test,
    Build,
    Delete,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientActionSubjectKind {
    RegisteredLanguageSource,
    RegisteredLanguageSourcePattern,
    ProviderConfigFile,
    Directory,
    StructuralSelector,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientActionAuthority {
    RawHostAction,
    RawShell,
    ParserOwnedExactEvidence,
    ParserOwnedSearch,
    AstPatchEvidence,
    Unknown,
}

/// Rule match axes from project-local hook config.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleMatchConfig {
    #[serde(default)]
    pub authority_rules: Vec<super::invocation::AgentActionAuthorityRule>,
    #[serde(default)]
    pub effect_rules: Vec<super::invocation::AgentActionEffectRule>,
    #[serde(default)]
    pub command_wrappers: Vec<super::invocation::HookClientCommandWrapper>,
    #[serde(default)]
    pub invocation_shape_any: Vec<super::invocation::HookClientInvocationShape>,
    #[serde(default)]
    pub wrapper_match_any: Vec<super::invocation::HookClientWrapperMatch>,
    #[serde(default)]
    pub flag_presence_any: Vec<super::invocation::HookClientFlagPresence>,
    #[serde(default)]
    pub action_any: Vec<HookClientActionKind>,
    #[serde(default)]
    pub effect_any: Vec<HookClientActionKind>,
    #[serde(default)]
    pub subject_kind_any: Vec<HookClientActionSubjectKind>,
    #[serde(default)]
    pub authority_any: Vec<HookClientActionAuthority>,
    #[serde(default)]
    pub authority_exclude_any: Vec<HookClientActionAuthority>,
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
    #[serde(default)]
    pub command_contains_any: Vec<String>,
    #[serde(default)]
    pub path_any: Vec<String>,
    #[serde(default)]
    pub path_glob_any: Vec<String>,
    #[serde(default)]
    pub argv_source_any: Vec<String>,
    #[serde(default)]
    pub argv_source_glob_any: Vec<String>,
    #[serde(default)]
    pub argv_source_exclude_flag_any: Vec<String>,
    /// Match when a parsed command stage carries a regular file owned by the workspace.
    #[serde(default)]
    pub argv_workspace_regular_file: bool,
    /// Match source paths owned by any activated language harness coverage contract.
    #[serde(default)]
    pub argv_registered_source_file: bool,
    /// Complete parser-owned structured projection matcher and lazy capability declaration.
    #[serde(default)]
    pub structured_projection: Option<HookClientStructuredProjectionMatchConfig>,
}

/// Structured document formats understood by hook projector capabilities.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientStructuredFormat {
    Json,
    Toml,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientStructuredProjectionMatchConfig {
    pub binary: String,
    pub document_format: HookClientStructuredFormat,
    pub filter_grammar: HookClientStructuredFilterGrammar,
    #[serde(default)]
    pub optional_subcommand_any: Vec<String>,
    #[serde(default)]
    pub option_any: Vec<String>,
    #[serde(default)]
    pub option_value_arity: std::collections::BTreeMap<String, u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientStructuredFilterGrammar {
    BoundedPathV1,
}

/// Route suggestion from project-local hook config.
#[derive(Debug, Deserialize)]
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
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigDecision {
    Allow,
    Block,
    Deny,
}

/// Config-level reason category spelling for a rule.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigReasonKind {
    None,
    DirectSourceRead,
    BulkSourceDump,
    RawBroadSearch,
    AgentSearchJson,
    SubagentReceiptRequired,
}

/// Config-level route kind spelling for a rule route.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigRouteKind {
    Prime,
    Owner,
    Query,
    Lexical,
    Read,
    Deps,
    Api,
    Ingest,
    Tests,
    CheckChanged,
}

/// Config-level stdin handling spelling for a route.
#[derive(Clone, Copy, Debug, Deserialize)]
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
