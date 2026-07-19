use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientDecisionMaterializer {
    AgentSearchJson,
    PromptSearchStrategy,
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

/// Rule match axes from project-local hook config.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleMatchConfig {
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub tool_any: Vec<String>,
    #[serde(default)]
    pub command_any: Vec<String>,
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
