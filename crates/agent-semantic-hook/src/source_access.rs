//! Codex-internal source access decision packet models.

use crate::protocol::DecisionRoute;
use crate::protocol_activation::protocol_activation_manifest::HookRuntime;
use crate::source_selector::collect_source_selector_matches;
use serde::Serialize;
use std::fmt;

/// Schema id for serialized source-access decision packets.
pub const SOURCE_ACCESS_DECISION_SCHEMA_ID: SourceAccessSchemaId =
    SourceAccessSchemaId::new("agent.semantic-protocols.source-access.decision");
/// Schema version for serialized source-access decision packets.
pub const SOURCE_ACCESS_DECISION_SCHEMA_VERSION: SourceAccessVersion =
    SourceAccessVersion::new("1");
/// Protocol id for source-access decision semantics.
pub const SOURCE_ACCESS_PROTOCOL_ID: SourceAccessProtocolId =
    SourceAccessProtocolId::new("agent.semantic-protocols.source-access");
/// Protocol version for source-access decision semantics.
pub const SOURCE_ACCESS_PROTOCOL_VERSION: SourceAccessVersion = SourceAccessVersion::new("1");

/// Stable schema identifier carried by source-access packets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SourceAccessSchemaId(&'static str);

impl SourceAccessSchemaId {
    /// Creates a schema id from a static protocol string.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }
}

/// Stable protocol identifier carried by source-access packets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SourceAccessProtocolId(&'static str);

impl SourceAccessProtocolId {
    /// Creates a protocol id from a static protocol string.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }
}

/// Version token used by source-access schema and protocol fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SourceAccessVersion(&'static str);

impl SourceAccessVersion {
    /// Creates a version token from a static protocol string.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }
}

/// Provider identity that owns the semantic route for a source-access decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SourceAccessProviderId(String);

impl SourceAccessProviderId {
    /// Creates a provider identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the provider identity as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SourceAccessProviderId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SourceAccessProviderId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl PartialEq<&str> for SourceAccessProviderId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for SourceAccessProviderId {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl fmt::Display for SourceAccessProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

/// Client runtime that produced a source-access decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessClient {
    /// OpenAI Codex runtime.
    Codex,
}

/// Runtime boundary where source-access policy was applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessBoundary {
    /// Codex filesystem API boundary.
    CodexFsApi,
    /// Codex tool action boundary.
    CodexToolAction,
    /// Shell command preflight boundary.
    CodexShellPreflight,
    /// Shell output egress boundary.
    CodexShellEgress,
    /// Subprocess open boundary.
    CodexSubprocessOpen,
}

/// Source-access operation observed by the runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessOperation {
    /// Read one file.
    ReadFile,
    /// Read a directory listing.
    ReadDirectory,
    /// Search files.
    FileSearch,
    /// Spawn a process.
    SpawnProcess,
    /// Write to process stdin.
    ProcessStdin,
    /// Return tool output to the model.
    ToolOutput,
    /// Operation could not be classified.
    Unknown,
}

/// Enforcement strength used for a source-access decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessEnforcement {
    /// Hard runtime gate.
    Hard,
    /// Preflight command gate.
    Preflight,
    /// Output egress gate.
    Egress,
    /// Advisory policy signal.
    Advisory,
    /// No enforcement was applied.
    NotEnforced,
}

/// Source-access decision outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessDecisionKind {
    /// Allow the access.
    Allow,
    /// Deny the access before bytes are returned.
    Deny,
    /// Suppress model-visible bytes after execution.
    Suppress,
    /// Return guidance instead of source bytes.
    Guide,
    /// Record the event without blocking.
    Observe,
}

/// Reason category for a source-access decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessReasonKind {
    /// No policy reason applies.
    None,
    /// Direct source read was attempted.
    DirectSourceRead,
    /// Bulk source dump was attempted.
    BulkSourceDump,
    /// Broad raw search was attempted.
    RawBroadSearch,
    /// Agent search JSON path was used.
    AgentSearchJson,
    /// Provider explicitly authorized compact source access.
    ProviderAuthorized,
    /// Target is not source code.
    NonSource,
    /// Boundary was observed but not enforced.
    NotEnforced,
    /// Generic policy reason.
    Policy,
}

/// Authorization source associated with a source-access decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessAuthorization {
    /// No authorization was present.
    None,
    /// Provider capability authorized the route.
    ProviderCapability,
    /// User approval authorized the route.
    UserApproved,
    /// Non-source target authorized the route.
    NonSource,
}

/// Subject observed at the source-access boundary.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAccessSubject {
    /// Tool name that requested or returned the bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// RPC method associated with a filesystem API access.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpc_method: Option<String>,
    /// Shell or provider command associated with the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Paths associated with the attempted source access.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Digest of source-like output hidden from the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_digest: Option<String>,
}

/// Provider route the agent should use instead of raw source access.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAccessRoute {
    /// Language id for the provider route.
    pub language_id: String,
    /// Provider id for the semantic route.
    pub provider_id: SourceAccessProviderId,
    /// Binary the agent should execute.
    pub binary: String,
    /// Provider route kind.
    pub kind: SourceAccessRouteKind,
    /// Full provider argv for the replacement route.
    pub argv: Vec<String>,
}

/// Semantic provider route kind for source-access repair.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAccessRouteKind {
    /// Prime search route.
    Prime,
    /// Owner search route.
    Owner,
    /// Query route.
    Query,
    /// Lexical search route.
    Lexical,
    /// Ingest route.
    Ingest,
    /// Guide route.
    Guide,
}

/// Source-access decision packet returned by the hook runtime.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAccessDecision {
    /// Source-access decision schema id.
    pub schema_id: SourceAccessSchemaId,
    /// Source-access decision schema version.
    pub schema_version: SourceAccessVersion,
    /// Source-access protocol id.
    pub protocol_id: SourceAccessProtocolId,
    /// Source-access protocol version.
    pub protocol_version: SourceAccessVersion,
    /// Client runtime that made the decision.
    pub client: SourceAccessClient,
    /// Runtime boundary where the decision was applied.
    pub boundary: SourceAccessBoundary,
    /// Operation observed at the boundary.
    pub operation: SourceAccessOperation,
    /// Enforcement strength for the decision.
    pub enforcement: SourceAccessEnforcement,
    /// Decision outcome.
    pub decision: SourceAccessDecisionKind,
    /// Reason category for the decision.
    pub reason_kind: SourceAccessReasonKind,
    /// Whether source bytes were returned by the underlying action.
    pub source_bytes_returned: bool,
    /// Whether source bytes were returned to the model.
    pub model_visible_bytes_returned: bool,
    /// Authorization source, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<SourceAccessAuthorization>,
    /// Language ids associated with the decision.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub language_ids: Vec<String>,
    /// Provider that authorized or should handle the source route.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<SourceAccessProviderId>,
    /// Subject that attempted or returned source access.
    pub subject: SourceAccessSubject,
    /// Replacement semantic routes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<SourceAccessRoute>,
    /// Short model-facing message.
    pub message: String,
    /// Additional model-facing notes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Named input for an explicitly requested filesystem source read.
#[derive(Debug)]
pub struct SourceAccessExplicitReadInput {
    /// Language id for the requested source path.
    pub language_id: String,
    /// Provider that owns the source path.
    pub provider_id: SourceAccessProviderId,
    /// Filesystem RPC method that attempted the read.
    pub rpc_method: String,
    /// Source path requested by the developer.
    pub path: String,
}

/// Named input for source-like shell output suppression.
#[derive(Debug)]
pub struct SourceAccessShellEgressSuppressedInput {
    /// Typed provider discovery route for recovering parser-owned identity.
    pub route: DecisionRoute,
    /// Shell command whose output was suppressed.
    pub command: String,
    /// Source path that appeared in command output.
    pub path: String,
    /// Digest of the suppressed output.
    pub output_digest: String,
}

/// Named input for provider-authorized compact source access.
#[derive(Debug)]
pub struct SourceAccessProviderCapabilityAllowInput {
    /// Language id for the provider route.
    pub language_id: String,
    /// Provider that authorized compact source access.
    pub provider_id: SourceAccessProviderId,
    /// Provider command that returned compact source facts.
    pub command: String,
    /// Source path covered by the provider command.
    pub path: String,
}

impl SourceAccessDecision {
    /// Builds an allow packet for an explicitly requested filesystem source read.
    pub fn explicit_read_allow(input: SourceAccessExplicitReadInput) -> Self {
        let language_id = input.language_id;
        let provider_id = input.provider_id;
        let path = input.path;
        Self {
            schema_id: SOURCE_ACCESS_DECISION_SCHEMA_ID,
            schema_version: SOURCE_ACCESS_DECISION_SCHEMA_VERSION,
            protocol_id: SOURCE_ACCESS_PROTOCOL_ID,
            protocol_version: SOURCE_ACCESS_PROTOCOL_VERSION,
            client: SourceAccessClient::Codex,
            boundary: SourceAccessBoundary::CodexFsApi,
            operation: SourceAccessOperation::ReadFile,
            enforcement: SourceAccessEnforcement::NotEnforced,
            decision: SourceAccessDecisionKind::Allow,
            reason_kind: SourceAccessReasonKind::None,
            source_bytes_returned: true,
            model_visible_bytes_returned: true,
            authorization: Some(SourceAccessAuthorization::UserApproved),
            language_ids: vec![language_id.clone()],
            provider_id: Some(provider_id.clone()),
            subject: SourceAccessSubject {
                rpc_method: Some(input.rpc_method),
                paths: vec![path.clone()],
                ..SourceAccessSubject::default()
            },
            routes: Vec::new(),
            message: format!("explicit source read allowed for {path}"),
            notes: Vec::new(),
        }
    }

    /// Builds an egress-suppression packet for source-like shell output.
    pub fn shell_egress_suppressed(input: SourceAccessShellEgressSuppressedInput) -> Self {
        let route = SourceAccessRoute::from(input.route);
        let language_id = route.language_id.clone();
        let provider_id = route.provider_id.clone();
        let path = input.path;
        Self {
            schema_id: SOURCE_ACCESS_DECISION_SCHEMA_ID,
            schema_version: SOURCE_ACCESS_DECISION_SCHEMA_VERSION,
            protocol_id: SOURCE_ACCESS_PROTOCOL_ID,
            protocol_version: SOURCE_ACCESS_PROTOCOL_VERSION,
            client: SourceAccessClient::Codex,
            boundary: SourceAccessBoundary::CodexShellEgress,
            operation: SourceAccessOperation::ToolOutput,
            enforcement: SourceAccessEnforcement::Egress,
            decision: SourceAccessDecisionKind::Suppress,
            reason_kind: SourceAccessReasonKind::BulkSourceDump,
            source_bytes_returned: true,
            model_visible_bytes_returned: false,
            authorization: Some(SourceAccessAuthorization::None),
            language_ids: vec![language_id.clone()],
            provider_id: Some(provider_id.clone()),
            subject: SourceAccessSubject {
                tool_name: Some("Bash".to_string()),
                command: Some(input.command),
                paths: vec![path.clone()],
                output_digest: Some(input.output_digest),
                ..SourceAccessSubject::default()
            },
            routes: vec![route],
            message: format!(
                "bulk-source-dump suppressed; use provider discovery to materialize a structural selector for {path}"
            ),
            notes: Vec::new(),
        }
    }

    /// Builds an allow packet for compact source access through a provider capability.
    pub fn provider_capability_allow(input: SourceAccessProviderCapabilityAllowInput) -> Self {
        let language_id = input.language_id;
        let provider_id = input.provider_id;
        Self {
            schema_id: SOURCE_ACCESS_DECISION_SCHEMA_ID,
            schema_version: SOURCE_ACCESS_DECISION_SCHEMA_VERSION,
            protocol_id: SOURCE_ACCESS_PROTOCOL_ID,
            protocol_version: SOURCE_ACCESS_PROTOCOL_VERSION,
            client: SourceAccessClient::Codex,
            boundary: SourceAccessBoundary::CodexToolAction,
            operation: SourceAccessOperation::ReadFile,
            enforcement: SourceAccessEnforcement::Hard,
            decision: SourceAccessDecisionKind::Allow,
            reason_kind: SourceAccessReasonKind::ProviderAuthorized,
            source_bytes_returned: true,
            model_visible_bytes_returned: true,
            authorization: Some(SourceAccessAuthorization::ProviderCapability),
            language_ids: vec![language_id],
            provider_id: Some(provider_id),
            subject: SourceAccessSubject {
                tool_name: Some("asp".to_string()),
                command: Some(input.command),
                paths: vec![input.path],
                ..SourceAccessSubject::default()
            },
            routes: Vec::new(),
            message: "provider-capability allowed compact source access.".to_string(),
            notes: Vec::new(),
        }
    }
}

/// Builds a Codex filesystem read decision when the registry owns the source path.
pub fn codex_fs_read_file_decision(
    registry: &HookRuntime,
    rpc_method: impl Into<String>,
    path: impl AsRef<str>,
) -> Option<SourceAccessDecision> {
    let path = path.as_ref();
    let matched = collect_source_selector_matches(registry, [path], |provider| {
        provider.policy.blocks_direct_source_read()
    })
    .into_iter()
    .next()?;
    let language_id = matched.provider.language_id.clone();
    let provider_id = matched.provider.provider_id.clone();
    Some(SourceAccessDecision::explicit_read_allow(
        SourceAccessExplicitReadInput {
            language_id,
            provider_id: SourceAccessProviderId(provider_id),
            rpc_method: rpc_method.into(),
            path: path.to_string(),
        },
    ))
}

/// Builds a Codex shell egress decision when source-like output must be hidden.
pub fn codex_shell_egress_suppression_decision(
    registry: &HookRuntime,
    command: impl Into<String>,
    path: impl AsRef<str>,
    output_digest: impl Into<String>,
) -> Option<SourceAccessDecision> {
    let path = path.as_ref();
    let matched = collect_source_selector_matches(registry, [path], |provider| {
        provider.policy.blocks_bulk_source_dump()
    })
    .into_iter()
    .next()?;
    let route = match matched.kind {
        SourceSelectorKind::ExactPath => matched.provider.route_from_template(
            DecisionRouteKind::Owner,
            &matched.provider.routes.owner,
            Some(&matched.route_selector),
            None,
        ),
        SourceSelectorKind::Pattern => matched.provider.route_from_template(
            DecisionRouteKind::Lexical,
            &matched.provider.routes.lexical,
            Some(&matched.route_selector),
            Some(&matched.route_selector),
        ),
    };
    Some(SourceAccessDecision::shell_egress_suppressed(
        SourceAccessShellEgressSuppressedInput {
            route,
            command: command.into(),
            path: path.to_string(),
            output_digest: output_digest.into(),
        },
    ))
}

impl From<DecisionRoute> for SourceAccessRoute {
    fn from(route: DecisionRoute) -> Self {
        Self {
            language_id: route.language_id,
            provider_id: route.provider_id.into(),
            binary: route.binary,
            kind: match route.kind {
                crate::protocol::DecisionRouteKind::Prime => SourceAccessRouteKind::Prime,
                crate::protocol::DecisionRouteKind::Owner => SourceAccessRouteKind::Owner,
                crate::protocol::DecisionRouteKind::Query
                | crate::protocol::DecisionRouteKind::Read => SourceAccessRouteKind::Query,
                crate::protocol::DecisionRouteKind::Lexical => SourceAccessRouteKind::Lexical,
                crate::protocol::DecisionRouteKind::Ingest => SourceAccessRouteKind::Ingest,
                crate::protocol::DecisionRouteKind::Deps
                | crate::protocol::DecisionRouteKind::Api
                | crate::protocol::DecisionRouteKind::Tests
                | crate::protocol::DecisionRouteKind::CheckChanged => SourceAccessRouteKind::Guide,
            },
            argv: route.argv,
        }
    }
}
use crate::{DecisionRouteKind, SourceSelectorKind};
