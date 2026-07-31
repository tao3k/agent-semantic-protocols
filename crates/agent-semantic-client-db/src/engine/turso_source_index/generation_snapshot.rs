use super::canonical::TursoSourceIndexCanonicalSelectorFact;
use super::prepare::TursoSourceIndexOwnerRow;
use agent_semantic_client_core::{SemanticSchemaId, SemanticSchemaVersion};
use agent_semantic_content_identity::SourceSnapshotEvidence;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ClientDbSourceIndexGenerationSnapshotError {
    FileHashesDecode(String),
    SourceSnapshotDecode(String),
    Incomplete {
        leaf_count: usize,
        file_hash_count: usize,
        owner_count: u32,
        owner_row_count: usize,
    },
    OwnerDigestOutsideGeneration {
        owner_path: String,
    },
    SelectorFactsDecode {
        owner_path: String,
        error: String,
    },
    OwnerSelectorCountDrift {
        owner_path: String,
        expected: usize,
        actual: usize,
    },
    GenerationSelectorCountDrift {
        expected: u32,
        actual: usize,
    },
}

impl std::fmt::Display for ClientDbSourceIndexGenerationSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileHashesDecode(error) => {
                write!(
                    formatter,
                    "failed to decode generation file hashes: {error}"
                )
            }
            Self::SourceSnapshotDecode(error) => {
                write!(
                    formatter,
                    "failed to decode generation source snapshot: {error}"
                )
            }
            Self::Incomplete {
                leaf_count,
                file_hash_count,
                owner_count,
                owner_row_count,
            } => write!(
                formatter,
                "generation is incomplete: leafCount={leaf_count} fileHashCount={file_hash_count} ownerCount={owner_count} ownerRowCount={owner_row_count}"
            ),
            Self::OwnerDigestOutsideGeneration { owner_path } => write!(
                formatter,
                "owner digest is outside generation: ownerPath={owner_path}"
            ),
            Self::SelectorFactsDecode { owner_path, error } => write!(
                formatter,
                "failed to decode canonical selectors: ownerPath={owner_path} error={error}"
            ),
            Self::OwnerSelectorCountDrift {
                owner_path,
                expected,
                actual,
            } => write!(
                formatter,
                "owner selector count drift: ownerPath={owner_path} expected={expected} actual={actual}"
            ),
            Self::GenerationSelectorCountDrift { expected, actual } => write!(
                formatter,
                "generation selector count drift: expected={expected} actual={actual}"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Canonical selector fact persisted for one source-index generation.
pub struct ClientDbSourceIndexSelectorFact {
    /// Stable selector identifier persisted by the language provider.
    pub selector_id: String,
    /// Optional parser-owned symbol.
    pub symbol: Option<String>,
    /// Optional parser-owned item kind.
    pub kind: Option<String>,
    /// Complete parser-owned exact-selector materialization proof.
    pub materialization_proof: agent_semantic_content_identity::ExactSelectorMaterializationProofV1,
    /// Canonical query keys associated with the selector.
    pub query_keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One owner and its selectors from an immutable source-index generation.
pub struct ClientDbSourceIndexGenerationOwner {
    /// Workspace-relative owner path.
    pub owner_path: String,
    /// Content digest committed into the generation root.
    pub owner_content_digest: String,
    /// Registered language identifier.
    pub language_id: Option<String>,
    /// Activated provider identifier.
    pub provider_id: Option<String>,
    /// Provider source-kind classification.
    pub source_kind: String,
    /// Optional source line count.
    pub line_count: Option<i64>,
    /// Canonical selectors attributed to the owner.
    pub selectors: Vec<ClientDbSourceIndexSelectorFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Fully materialized, root-verified active source-index generation.
pub struct ClientDbSourceIndexGenerationSnapshot {
    /// Immutable generation identifier.
    pub generation_id: String,
    /// Complete workspace owner path to content digest map.
    pub file_hashes: BTreeMap<String, String>,
    /// Merkle evidence published with the generation.
    pub source_snapshot: SourceSnapshotEvidence,
    /// Published owner count.
    pub owner_count: u32,
    /// Published selector count.
    pub selector_count: u32,
    /// Deterministically ordered owner facts.
    pub owners: Vec<ClientDbSourceIndexGenerationOwner>,
}

/// Load one complete active generation and reject partial Merkle or selector coverage.
pub async fn latest_turso_source_index_generation_snapshot(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<ClientDbSourceIndexGenerationSnapshot>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let normalized_project_root = crate::types::normalized_project_root(project_root)?;
    let connection = crate::engine::turso::connect_turso_client_db(db_path).await?;
    super::core::ensure_turso_source_index_schema(&connection).await?;
    load_turso_source_index_generation_snapshot(
        &connection,
        normalized_project_root.as_str(),
        schema_id.as_str(),
        schema_version.as_str(),
    )
    .await
}

pub(super) async fn latest_turso_source_index_generation_on_connection(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<Option<(String, String, String, u32, u32)>, String> {
    let mut rows = connection
        .query(
            "SELECT generation_id, file_hashes_json, source_snapshot_json, owner_count, selector_count
                     FROM asp_source_index_scope_v1
WHERE project_root = ?1
  AND schema_id = ?2
  AND schema_version = ?3
  AND source_snapshot_json <> ''
ORDER BY updated_at_ms DESC, generation_id DESC
LIMIT 1",
            (project_root, schema_id, schema_version),
        )
        .await
        .map_err(|error| format!("failed to query latest Turso source-index generation: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read latest Turso source-index generation: {error}"))?
    else {
        return Ok(None);
    };
    Ok(Some((
        row.get::<String>(0)
            .map_err(|error| format!("failed to read Turso source-index generation id: {error}"))?,
        row.get::<String>(1)
            .map_err(|error| format!("failed to read Turso source-index file hashes: {error}"))?,
        row.get::<String>(2).map_err(|error| {
            format!("failed to read Turso source-index source snapshot evidence: {error}")
        })?,
        row.get::<i64>(3)
            .map_err(|error| format!("failed to read Turso source-index owner count: {error}"))?
            .max(0)
            .min(i64::from(u32::MAX)) as u32,
        row.get::<i64>(4)
            .map_err(|error| format!("failed to read Turso source-index selector count: {error}"))?
            .max(0)
            .min(i64::from(u32::MAX)) as u32,
    )))
}

pub(super) async fn load_turso_source_index_generation_snapshot(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<Option<ClientDbSourceIndexGenerationSnapshot>, String> {
    let Some((generation_id, file_hashes_json, source_snapshot_json, owner_count, selector_count)) =
        latest_turso_source_index_generation_on_connection(
            connection,
            project_root,
            schema_id,
            schema_version,
        )
        .await?
    else {
        return Ok(None);
    };
    let (owner_generation_id, owner_rows) = super::prepare::active_turso_source_index_owner_rows(
        connection,
        project_root,
        schema_id,
        schema_version,
    )
    .await?;
    if owner_generation_id != generation_id {
        return Err(format!(
            "Turso source-index generation changed during snapshot load: scope={generation_id} owners={owner_generation_id}"
        ));
    }
    materialize_turso_source_index_generation_snapshot(
        generation_id,
        &file_hashes_json,
        &source_snapshot_json,
        owner_count,
        selector_count,
        owner_rows,
    )
    .map_err(|error| format!("failed to materialize Turso source-index generation: {error}"))
    .map(Some)
}

pub(super) fn materialize_turso_source_index_generation_snapshot(
    generation_id: String,
    file_hashes_json: &str,
    source_snapshot_json: &str,
    owner_count: u32,
    selector_count: u32,
    owner_rows: HashMap<String, TursoSourceIndexOwnerRow>,
) -> Result<ClientDbSourceIndexGenerationSnapshot, ClientDbSourceIndexGenerationSnapshotError> {
    let file_hash_records = serde_json::from_str::<
        Vec<agent_semantic_client_core::ClientCacheFileHash>,
    >(file_hashes_json)
    .map_err(|error| {
        ClientDbSourceIndexGenerationSnapshotError::FileHashesDecode(error.to_string())
    })?;
    let file_hash_record_count = file_hash_records.len();
    let all_file_hashes = file_hash_records
        .into_iter()
        .map(|file_hash| (file_hash.path, file_hash.sha256))
        .collect::<BTreeMap<_, _>>();
    let file_hashes = owner_rows
        .keys()
        .filter_map(|owner_path| {
            all_file_hashes
                .get(owner_path)
                .map(|digest| (owner_path.clone(), digest.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let source_snapshot = serde_json::from_str::<SourceSnapshotEvidence>(source_snapshot_json)
        .map_err(|error| {
            ClientDbSourceIndexGenerationSnapshotError::SourceSnapshotDecode(error.to_string())
        })?;
    if source_snapshot.leaf_count != file_hashes.len()
        || all_file_hashes.len() != file_hash_record_count
        || file_hashes.len() != owner_count as usize
        || owner_rows.len() != owner_count as usize
    {
        return Err(ClientDbSourceIndexGenerationSnapshotError::Incomplete {
            leaf_count: source_snapshot.leaf_count,
            file_hash_count: file_hashes.len(),
            owner_count,
            owner_row_count: owner_rows.len(),
        });
    }
    let mut decoded_selector_count = 0usize;
    let mut owners = owner_rows
        .into_values()
        .map(|owner| {
            if file_hashes.get(owner.owner_path.as_str()) != Some(&owner.file_hash) {
                return Err(
                    ClientDbSourceIndexGenerationSnapshotError::OwnerDigestOutsideGeneration {
                        owner_path: owner.owner_path,
                    },
                );
            }
            let selector_facts =
                serde_json::from_str::<Vec<TursoSourceIndexCanonicalSelectorFact>>(
                    &owner.selector_facts_json,
                )
                .map_err(|error| {
                    ClientDbSourceIndexGenerationSnapshotError::SelectorFactsDecode {
                        owner_path: owner.owner_path.clone(),
                        error: error.to_string(),
                    }
                })?;
            if selector_facts.len() != owner.selector_count.max(0) as usize {
                return Err(
                    ClientDbSourceIndexGenerationSnapshotError::OwnerSelectorCountDrift {
                        owner_path: owner.owner_path,
                        expected: owner.selector_count.max(0) as usize,
                        actual: selector_facts.len(),
                    },
                );
            }
            decoded_selector_count += selector_facts.len();
            Ok(ClientDbSourceIndexGenerationOwner {
                owner_path: owner.owner_path,
                owner_content_digest: owner.file_hash,
                language_id: owner.language_id,
                provider_id: owner.provider_id,
                source_kind: owner.source_kind,
                line_count: owner.line_count,
                selectors: selector_facts
                    .into_iter()
                    .map(|selector| ClientDbSourceIndexSelectorFact {
                        selector_id: selector.selector_id,
                        symbol: selector.symbol,
                        kind: selector.kind,
                        materialization_proof: selector.materialization_proof,
                        query_keys: selector.query_keys,
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, ClientDbSourceIndexGenerationSnapshotError>>()?;
    if decoded_selector_count != selector_count as usize {
        return Err(
            ClientDbSourceIndexGenerationSnapshotError::GenerationSelectorCountDrift {
                expected: selector_count,
                actual: decoded_selector_count,
            },
        );
    }
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    Ok(ClientDbSourceIndexGenerationSnapshot {
        generation_id,
        file_hashes,
        source_snapshot,
        owner_count,
        selector_count,
        owners,
    })
}
