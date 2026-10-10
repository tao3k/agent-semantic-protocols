// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;

use agent_semantic_content_identity::CanonicalItemSelector;

use super::workspace_db_registry::ProviderSearchWorkspaceSession;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TursoResidentSelectorQuery {
    pub project_root: String,
    pub schema_id: String,
    pub schema_version: String,
    pub provider_id: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub owner_path: String,
    pub canonical_item_selector: CanonicalItemSelector,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TursoResidentSelectorCandidate {
    pub owner_path: String,
    pub owner_content_digest: String,
    pub canonical_item_selector: CanonicalItemSelector,
    pub projection: Option<super::ProviderSelectorProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TursoResidentSelectorRead {
    pub generation_id: String,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub owner_count: u32,
    pub selector_count: u32,
    pub requested_owner_exists: bool,
    pub candidates: Vec<TursoResidentSelectorCandidate>,
    pub actual_kinds: Vec<String>,
    pub database_query_count: u32,
}

pub(super) async fn read_turso_resident_selector(
    session: &ProviderSearchWorkspaceSession,
    request: &TursoResidentSelectorQuery,
) -> Result<Option<TursoResidentSelectorRead>, String> {
    request
        .canonical_item_selector
        .validate()
        .map_err(|error| format!("invalid resident selector query: {error}"))?;
    for (field, value) in [
        ("projectRoot", request.project_root.as_str()),
        ("schemaId", request.schema_id.as_str()),
        ("schemaVersion", request.schema_version.as_str()),
        ("providerId", request.provider_id.as_str()),
        (
            "parserIdentityDigest",
            request.parser_identity_digest.as_str(),
        ),
        ("queryPackDigest", request.query_pack_digest.as_str()),
        ("ownerPath", request.owner_path.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("resident selector query {field} must not be empty"));
        }
    }
    let scopes_json = serde_json::to_string(&request.canonical_item_selector.scopes)
        .map_err(|error| format!("failed to encode resident selector scopes: {error}"))?;
    let mut rows = session
        .read_connection()
        .query(
            "SELECT active.generation_id,
                    active.source_snapshot_json,
                    active.owner_count,
                    active.selector_count,
                    selector.owner_path,
                    selector.owner_content_digest,
                    selector.item_kind,
                    selector.structural_selector,
                    projection.capture_name,
                    projection.signature,
                    projection.item_kind,
                    projection.item_name,
                    projection.source_byte_start,
                    projection.source_byte_end,
                    EXISTS (
                        SELECT 1
                        FROM asp_source_index_owner_v1 AS requested_owner
                        WHERE requested_owner.project_root = active.project_root
                          AND requested_owner.schema_id = active.schema_id
                          AND requested_owner.schema_version = active.schema_version
                          AND requested_owner.generation_id = active.generation_id
                          AND requested_owner.owner_path = ?10
                    )
             FROM asp_source_index_scope_v1 AS active
             LEFT JOIN asp_source_index_selector_v1 AS selector
               ON selector.project_root = active.project_root
              AND selector.schema_id = active.schema_id
              AND selector.schema_version = active.schema_version
              AND selector.generation_id = active.generation_id
              AND selector.language_id = ?4
              AND selector.parser_identity_digest = ?5
              AND selector.query_pack_digest = ?6
              AND selector.item_symbol = ?7
              AND selector.scopes_json = ?8
             LEFT JOIN provider_active_generation_v1 AS provider_active
               ON provider_active.project_root = active.project_root
              AND provider_active.provider_id = ?9
              AND provider_active.language_id = ?4
             LEFT JOIN provider_selector_projection_v1 AS projection
               ON projection.project_root = provider_active.project_root
              AND projection.workspace_identity = provider_active.workspace_identity
              AND projection.provider_workspace_identity_digest =
                  provider_active.provider_workspace_identity_digest
              AND projection.provider_id = provider_active.provider_id
              AND projection.owner_path = selector.owner_path
              AND projection.source_content_digest = selector.owner_content_digest
              AND projection.structural_selector = selector.structural_selector
             WHERE active.project_root = ?1
               AND active.schema_id = ?2
               AND active.schema_version = ?3
             ORDER BY selector.owner_path, selector.item_kind, selector.structural_selector",
            (
                request.project_root.as_str(),
                request.schema_id.as_str(),
                request.schema_version.as_str(),
                request.canonical_item_selector.language_id.as_str(),
                request.parser_identity_digest.as_str(),
                request.query_pack_digest.as_str(),
                request.canonical_item_selector.symbol.as_str(),
                scopes_json.as_str(),
                request.provider_id.as_str(),
                request.owner_path.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to query resident Turso selector index: {error}"))?;

    let mut generation = None;
    let mut candidates = Vec::new();
    let mut actual_kinds = BTreeSet::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read resident Turso selector index: {error}"))?
    {
        if generation.is_none() {
            let source_snapshot_json = row.get::<String>(1).map_err(|error| {
                format!("failed to decode resident Turso source snapshot: {error}")
            })?;
            generation = Some((
                row.get::<String>(0).map_err(|error| {
                    format!("failed to decode resident Turso generation: {error}")
                })?,
                serde_json::from_str(&source_snapshot_json).map_err(|error| {
                    format!("failed to parse resident Turso source snapshot: {error}")
                })?,
                row.get::<i64>(2)
                    .map_err(|error| {
                        format!("failed to decode resident Turso owner count: {error}")
                    })?
                    .max(0)
                    .min(i64::from(u32::MAX)) as u32,
                row.get::<i64>(3)
                    .map_err(|error| {
                        format!("failed to decode resident Turso selector count: {error}")
                    })?
                    .max(0)
                    .min(i64::from(u32::MAX)) as u32,
                row.get::<i64>(14).map_err(|error| {
                    format!("failed to decode requested owner membership: {error}")
                })? != 0,
            ));
        }
        let owner_path = row
            .get::<Option<String>>(4)
            .map_err(|error| format!("failed to decode resident Turso selector owner: {error}"))?;
        let Some(item_kind) = row
            .get::<Option<String>>(6)
            .map_err(|error| format!("failed to decode resident Turso item kind: {error}"))?
        else {
            continue;
        };
        let owner_path = owner_path.ok_or_else(|| {
            "resident Turso selector row has item kind without owner identity".to_owned()
        })?;
        if owner_path == request.owner_path {
            actual_kinds.insert(item_kind.clone());
        }
        if item_kind != request.canonical_item_selector.kind.as_str() {
            continue;
        }
        let structural_selector = row.get::<String>(7).map_err(|error| {
            format!("failed to decode resident Turso structural selector: {error}")
        })?;
        let projection = match row.get::<Option<String>>(8).map_err(|error| {
            format!("failed to decode resident Turso projection capture: {error}")
        })? {
            Some(capture_name) => Some(super::ProviderSelectorProjection {
                structural_selector: structural_selector.clone(),
                capture_name,
                signature: row.get::<String>(9).map_err(|error| {
                    format!("failed to decode resident Turso projection signature: {error}")
                })?,
                item_kind: row.get::<String>(10).map_err(|error| {
                    format!("failed to decode resident Turso projection item kind: {error}")
                })?,
                item_name: row.get::<String>(11).map_err(|error| {
                    format!("failed to decode resident Turso projection item name: {error}")
                })?,
                source_byte_start: row
                    .get::<i64>(12)
                    .map_err(|error| {
                        format!("failed to decode resident Turso projection start: {error}")
                    })?
                    .max(0) as u64,
                source_byte_end: row
                    .get::<i64>(13)
                    .map_err(|error| {
                        format!("failed to decode resident Turso projection end: {error}")
                    })?
                    .max(0) as u64,
            }),
            None => None,
        };
        candidates.push(TursoResidentSelectorCandidate {
            owner_path,
            owner_content_digest: row.get::<String>(5).map_err(|error| {
                format!("failed to decode resident Turso owner digest: {error}")
            })?,
            canonical_item_selector: CanonicalItemSelector::parse(structural_selector)
                .map_err(|error| format!("resident Turso selector identity is invalid: {error}"))?,
            projection,
        });
    }
    let Some((generation_id, source_snapshot, owner_count, selector_count, requested_owner_exists)) =
        generation
    else {
        return Ok(None);
    };
    Ok(Some(TursoResidentSelectorRead {
        generation_id,
        source_snapshot,
        owner_count,
        selector_count,
        requested_owner_exists,
        candidates,
        actual_kinds: actual_kinds.into_iter().collect(),
        database_query_count: 1,
    }))
}
