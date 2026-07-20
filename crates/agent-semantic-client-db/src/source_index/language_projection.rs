//! Query-free language-harness projections consumed by the source-index lifecycle.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use agent_semantic_client_core::{ClientCacheFileHash, LanguageId, ProviderId};
use serde::Deserialize;

use super::types::{
    ClientDbSourceIndexOwner, ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorPayloadProof, ClientDbSourceIndexSource,
};

pub const CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-language-projection";
pub const CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_VERSION: &str = "1";
pub const CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_ID: &str =
    "agent.semantic-protocols.language-projection";
pub const CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_VERSION: &str = "1";

/// Parser-owned facts published by one language harness without search state.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDbLanguageProjection {
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub language_id: String,
    pub harness: ClientDbLanguageProjectionHarness,
    pub sources: Vec<ClientDbLanguageProjectionSource>,
    pub owners: Vec<ClientDbLanguageProjectionOwner>,
    pub items: Vec<ClientDbLanguageProjectionItem>,
    pub relations: Vec<ClientDbLanguageProjectionRelation>,
}

/// Parser identity attached to a language projection.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDbLanguageProjectionHarness {
    pub harness_id: String,
    pub parser_abi: String,
    pub selector_dialect: String,
}

/// One parser-owned source identity. Digests belong to the ASP lifecycle.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDbLanguageProjectionSource {
    pub source_id: String,
    pub path: String,
    pub source_kind: ClientDbLanguageProjectionSourceKind,
}

/// Source classification emitted by a language harness.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientDbLanguageProjectionSourceKind {
    Source,
    Test,
    Fixture,
    Config,
    Generated,
}

/// Parser-owned owner identity.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDbLanguageProjectionOwner {
    pub owner_id: String,
    pub source_id: String,
    pub kind: Option<String>,
    pub name: Option<String>,
}

/// Parser-owned exact item selector.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDbLanguageProjectionItem {
    pub item_id: String,
    pub owner_id: String,
    pub kind: String,
    pub name: String,
    pub selector: String,
}

/// One typed relation preserved for EvidenceGraph import.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDbLanguageProjectionRelation {
    pub from: ClientDbLanguageProjectionNodeRef,
    pub kind: String,
    pub to: ClientDbLanguageProjectionNodeRef,
}

/// Reference to a parser-owned or external projection node.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDbLanguageProjectionNodeRef {
    pub kind: ClientDbLanguageProjectionNodeKind,
    pub id: String,
}

/// Node kinds admitted by the shared language projection contract.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientDbLanguageProjectionNodeKind {
    Source,
    Owner,
    Item,
    External,
}

/// Generic lifecycle input for importing parser-owned projection facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbLanguageProjectionImportRequest {
    pub project_root: PathBuf,
    pub previous_file_hashes: Option<Vec<ClientCacheFileHash>>,
    pub registry_fingerprint: String,
    pub projection: ClientDbLanguageProjection,
}

/// Content-addressed language projection prepared for source-index persistence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbLanguageProjectionImport {
    pub source_index: super::types::ClientDbSourceIndexImport,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
}

/// Rows derived from parser facts without reading or parsing language source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LanguageProjectionSourceIndexRows {
    pub scope_files: Vec<ClientDbSourceIndexScopeFile>,
    pub owners: Vec<ClientDbSourceIndexOwner>,
    pub selectors: Vec<ClientDbSourceIndexSelector>,
}

impl ClientDbLanguageProjection {
    /// Decode and validate a shared, query-free language projection artifact.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let projection: Self = serde_json::from_str(json)
            .map_err(|error| format!("invalid language projection JSON: {error}"))?;
        projection.validate()?;
        Ok(projection)
    }

    /// Validate schema identity and parser-owned foreign keys before import.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_ID
            || self.schema_version != CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_VERSION
            || self.protocol_id != CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_ID
            || self.protocol_version != CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_VERSION
        {
            return Err("unsupported language projection schema or protocol".to_string());
        }
        ensure_nonempty("languageId", &self.language_id)?;
        ensure_nonempty("harnessId", &self.harness.harness_id)?;
        ensure_nonempty("parserAbi", &self.harness.parser_abi)?;
        ensure_nonempty("selectorDialect", &self.harness.selector_dialect)?;
        if self.sources.is_empty() {
            return Err("language projection must contain at least one source".to_string());
        }

        let source_ids = unique_identifiers(
            "sourceId",
            self.sources.iter().map(|source| &source.source_id),
        )?;
        unique_identifiers(
            "source.path",
            self.sources.iter().map(|source| &source.path),
        )?;
        for source in &self.sources {
            ensure_safe_relative_path(&source.path)?;
        }
        let owner_ids =
            unique_identifiers("ownerId", self.owners.iter().map(|owner| &owner.owner_id))?;
        for owner in &self.owners {
            if !source_ids.contains(owner.source_id.as_str()) {
                return Err(format!(
                    "language projection owner {} references unknown source {}",
                    owner.owner_id, owner.source_id
                ));
            }
        }
        let item_ids = unique_identifiers("itemId", self.items.iter().map(|item| &item.item_id))?;
        for item in &self.items {
            if !owner_ids.contains(item.owner_id.as_str()) {
                return Err(format!(
                    "language projection item {} references unknown owner {}",
                    item.item_id, item.owner_id
                ));
            }
            ensure_nonempty("item.kind", &item.kind)?;
            ensure_nonempty("item.name", &item.name)?;
            ensure_nonempty("item.selector", &item.selector)?;
        }
        for relation in &self.relations {
            ensure_nonempty("relation.kind", &relation.kind)?;
            if language_projection_relation_kind(&relation.kind).is_none() {
                return Err(format!(
                    "unsupported language projection relation kind {}",
                    relation.kind
                ));
            }
            validate_node_ref(&relation.from, &source_ids, &owner_ids, &item_ids)?;
            validate_node_ref(&relation.to, &source_ids, &owner_ids, &item_ids)?;
        }
        Ok(())
    }
}

/// Convert parser facts into DB source-index rows without source-text projection.
pub(crate) fn language_projection_source_index_rows(
    projection: &ClientDbLanguageProjection,
    project_root: &Path,
) -> Result<LanguageProjectionSourceIndexRows, String> {
    projection.validate()?;
    let language_id = LanguageId::from(projection.language_id.clone());
    let provider_id = ProviderId::from(projection.harness.harness_id.clone());
    let owners_by_source = projection.owners.iter().fold(
        BTreeMap::<&str, Vec<&ClientDbLanguageProjectionOwner>>::new(),
        |mut grouped, owner| {
            grouped
                .entry(owner.source_id.as_str())
                .or_default()
                .push(owner);
            grouped
        },
    );
    let items_by_owner = projection.items.iter().fold(
        BTreeMap::<&str, Vec<&ClientDbLanguageProjectionItem>>::new(),
        |mut grouped, item| {
            grouped
                .entry(item.owner_id.as_str())
                .or_default()
                .push(item);
            grouped
        },
    );
    let mut scope_files = Vec::with_capacity(projection.sources.len());
    let mut owners = Vec::with_capacity(projection.sources.len());
    let mut selectors = Vec::with_capacity(projection.items.len());
    for source in &projection.sources {
        let source_owners = owners_by_source
            .get(source.source_id.as_str())
            .cloned()
            .unwrap_or_default();
        let source_items = source_owners
            .iter()
            .flat_map(|owner| {
                items_by_owner
                    .get(owner.owner_id.as_str())
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect::<Vec<_>>();
        let owner_path = ClientDbSourceIndexPath::from(source.path.clone());
        let query_keys = projection_query_keys(
            std::iter::once(source.path.as_str())
                .chain(
                    source_owners
                        .iter()
                        .filter_map(|owner| owner.kind.as_deref()),
                )
                .chain(
                    source_owners
                        .iter()
                        .filter_map(|owner| owner.name.as_deref()),
                )
                .chain(source_items.iter().map(|item| item.kind.as_str()))
                .chain(source_items.iter().map(|item| item.name.as_str())),
        );
        let source_selectors = source_items
            .iter()
            .map(|item| ClientDbSourceIndexSelector {
                owner_path: owner_path.clone(),
                selector_id: item.selector.clone(),
                symbol: Some(item.name.clone()),
                kind: Some(item.kind.clone()),
                start_line: 0,
                end_line: 0,
                source: ClientDbSourceIndexSource::from("harness-projection"),
                query_keys: projection_query_keys([item.kind.as_str(), item.name.as_str()]),
                payload_proof: Some(ClientDbSourceIndexSelectorPayloadProof {
                    structural_selector: item.selector.clone(),
                    payload_kind: "code".to_string(),
                    bounded: true,
                }),
            })
            .collect::<Vec<_>>();
        selectors.extend(source_selectors.iter().cloned());
        scope_files.push(ClientDbSourceIndexScopeFile {
            path: project_root.join(&source.path),
            language_id: language_id.clone(),
            provider_id: provider_id.clone(),
            selector_receipts: source_selectors,
        });
        owners.push(ClientDbSourceIndexOwner {
            owner_path,
            language_id: Some(language_id.clone()),
            provider_id: Some(provider_id.clone()),
            source_kind: ClientDbSourceIndexSource::from("harness-projection"),
            line_count: None,
            query_keys,
        });
    }
    Ok(LanguageProjectionSourceIndexRows {
        scope_files,
        owners,
        selectors,
    })
}

/// Project parser-owned semantic facts into the shared EvidenceGraph vocabulary.
pub(crate) fn language_projection_evidence_graph(
    import: &crate::ClientDbSourceIndexImport,
    projection: &ClientDbLanguageProjection,
) -> Result<crate::evidence_graph::ClientDbEvidenceGraph, String> {
    projection.validate()?;
    let mut graph = crate::evidence_graph::empty_evidence_graph(
        import.generation_id.as_str(),
        import.project_root.clone(),
    );
    let language_id = Some(projection.language_id.clone());
    let provider_id = Some(projection.harness.harness_id.clone());
    let mut node_ids = BTreeMap::<String, String>::new();
    let mut source_paths = BTreeMap::<String, String>::new();

    for source in &projection.sources {
        let node_id = language_projection_graph_node_id(
            import.generation_id.as_str(),
            "source",
            &source.source_id,
        );
        source_paths.insert(source.source_id.clone(), source.path.clone());
        node_ids.insert(
            language_projection_node_ref_key("source", &source.source_id),
            node_id.clone(),
        );
        graph
            .nodes
            .push(crate::evidence_graph::ClientDbEvidenceGraphNode {
                id: node_id,
                kind: "source-file",
                semantic_kind: None,
                label: source.path.clone(),
                path: Some(source.path.clone()),
                selector: None,
                query_keys: language_projection_graph_query_keys([source.path.as_str()]),
                language_id: language_id.clone(),
                provider_id: provider_id.clone(),
            });
    }
    for owner in &projection.owners {
        let source_path = source_paths.get(&owner.source_id).ok_or_else(|| {
            format!(
                "language projection owner {} is missing source {}",
                owner.owner_id, owner.source_id
            )
        })?;
        let node_id = language_projection_graph_node_id(
            import.generation_id.as_str(),
            "owner",
            &owner.owner_id,
        );
        node_ids.insert(
            language_projection_node_ref_key("owner", &owner.owner_id),
            node_id.clone(),
        );
        let mut query_key_values = vec![source_path.clone()];
        if let Some(kind) = &owner.kind {
            query_key_values.push(kind.clone());
        }
        if let Some(name) = &owner.name {
            query_key_values.push(name.clone());
        }
        graph
            .nodes
            .push(crate::evidence_graph::ClientDbEvidenceGraphNode {
                id: node_id,
                kind: "source-owner",
                semantic_kind: owner.kind.clone(),
                label: owner.name.clone().unwrap_or_else(|| source_path.clone()),
                path: Some(source_path.clone()),
                selector: None,
                query_keys: language_projection_graph_query_keys(
                    query_key_values.iter().map(String::as_str),
                ),
                language_id: language_id.clone(),
                provider_id: provider_id.clone(),
            });
    }
    for item in &projection.items {
        let owner = projection
            .owners
            .iter()
            .find(|owner| owner.owner_id == item.owner_id)
            .ok_or_else(|| {
                format!(
                    "language projection item {} is missing owner {}",
                    item.item_id, item.owner_id
                )
            })?;
        let source_path = source_paths.get(&owner.source_id).ok_or_else(|| {
            format!(
                "language projection item {} is missing source {}",
                item.item_id, owner.source_id
            )
        })?;
        let node_id =
            language_projection_graph_node_id(import.generation_id.as_str(), "item", &item.item_id);
        node_ids.insert(
            language_projection_node_ref_key("item", &item.item_id),
            node_id.clone(),
        );
        graph
            .nodes
            .push(crate::evidence_graph::ClientDbEvidenceGraphNode {
                id: node_id,
                kind: "selector",
                semantic_kind: Some(item.kind.clone()),
                label: item.name.clone(),
                path: Some(source_path.clone()),
                selector: Some(item.selector.clone()),
                query_keys: language_projection_graph_query_keys([
                    item.kind.as_str(),
                    item.name.as_str(),
                ]),
                language_id: language_id.clone(),
                provider_id: provider_id.clone(),
            });
    }
    for relation in &projection.relations {
        let from = language_projection_relation_node_id(
            import.generation_id.as_str(),
            &relation.from,
            &mut node_ids,
            &mut graph,
            language_id.as_deref(),
            provider_id.as_deref(),
        )?;
        let to = language_projection_relation_node_id(
            import.generation_id.as_str(),
            &relation.to,
            &mut node_ids,
            &mut graph,
            language_id.as_deref(),
            provider_id.as_deref(),
        )?;
        let kind = language_projection_relation_kind(&relation.kind).ok_or_else(|| {
            format!(
                "unsupported language projection relation kind {}",
                relation.kind
            )
        })?;
        graph
            .edges
            .push(crate::evidence_graph::ClientDbEvidenceGraphEdge { from, to, kind });
    }
    Ok(graph)
}

fn language_projection_relation_node_id(
    generation_id: &str,
    node: &ClientDbLanguageProjectionNodeRef,
    node_ids: &mut BTreeMap<String, String>,
    graph: &mut crate::evidence_graph::ClientDbEvidenceGraph,
    language_id: Option<&str>,
    provider_id: Option<&str>,
) -> Result<String, String> {
    let kind = match &node.kind {
        ClientDbLanguageProjectionNodeKind::Source => "source",
        ClientDbLanguageProjectionNodeKind::Owner => "owner",
        ClientDbLanguageProjectionNodeKind::Item => "item",
        ClientDbLanguageProjectionNodeKind::External => "external",
    };
    let key = language_projection_node_ref_key(kind, &node.id);
    if let Some(node_id) = node_ids.get(&key) {
        return Ok(node_id.clone());
    }
    if kind != "external" {
        return Err(format!("language projection graph is missing node {key}"));
    }
    let node_id = language_projection_graph_node_id(generation_id, kind, &node.id);
    node_ids.insert(key, node_id.clone());
    graph
        .nodes
        .push(crate::evidence_graph::ClientDbEvidenceGraphNode {
            id: node_id.clone(),
            kind: "external",
            semantic_kind: None,
            label: node.id.clone(),
            path: None,
            selector: None,
            query_keys: Vec::new(),
            language_id: language_id.map(str::to_string),
            provider_id: provider_id.map(str::to_string),
        });
    Ok(node_id)
}

fn language_projection_graph_node_id(generation_id: &str, kind: &str, id: &str) -> String {
    format!("language-projection:{generation_id}:{kind}:{id}")
}

fn language_projection_node_ref_key(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

fn language_projection_relation_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "contains" => Some("contains"),
        "depends-on" => Some("depends-on"),
        "tests" => Some("tests"),
        "references" => Some("references"),
        _ => None,
    }
}

fn language_projection_graph_query_keys<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    projection_query_keys(values)
        .into_iter()
        .map(|key| key.as_str().to_string())
        .collect()
}

fn projection_query_keys<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Vec<ClientDbSourceIndexQueryKey> {
    values
        .into_iter()
        .flat_map(|value| {
            value
                .split(|character: char| {
                    !character.is_ascii_alphanumeric() && character != '_' && character != '-'
                })
                .filter(|term| !term.is_empty())
                .map(str::to_ascii_lowercase)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(ClientDbSourceIndexQueryKey::from)
        .collect()
}

fn unique_identifiers<'a>(
    field: &str,
    identifiers: impl IntoIterator<Item = &'a String>,
) -> Result<BTreeSet<&'a str>, String> {
    let mut unique = BTreeSet::new();
    for identifier in identifiers {
        ensure_nonempty(field, identifier)?;
        if !unique.insert(identifier.as_str()) {
            return Err(format!(
                "language projection contains duplicate {field} {identifier}"
            ));
        }
    }
    Ok(unique)
}

fn validate_node_ref(
    node: &ClientDbLanguageProjectionNodeRef,
    source_ids: &BTreeSet<&str>,
    owner_ids: &BTreeSet<&str>,
    item_ids: &BTreeSet<&str>,
) -> Result<(), String> {
    ensure_nonempty("relation.node.id", &node.id)?;
    let known = match node.kind {
        ClientDbLanguageProjectionNodeKind::Source => source_ids.contains(node.id.as_str()),
        ClientDbLanguageProjectionNodeKind::Owner => owner_ids.contains(node.id.as_str()),
        ClientDbLanguageProjectionNodeKind::Item => item_ids.contains(node.id.as_str()),
        ClientDbLanguageProjectionNodeKind::External => true,
    };
    if known {
        Ok(())
    } else {
        Err(format!(
            "language projection relation references unknown node {}",
            node.id
        ))
    }
}

fn ensure_safe_relative_path(path: &str) -> Result<(), String> {
    ensure_nonempty("source.path", path)?;
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        Err(format!(
            "language projection source path must be relative: {path}"
        ))
    } else {
        Ok(())
    }
}

fn ensure_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("language projection {field} must not be empty"))
    } else {
        Ok(())
    }
}
