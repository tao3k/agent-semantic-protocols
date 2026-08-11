use std::collections::{BTreeMap, BTreeSet};

use agent_semantic_client_core::{LanguageId, ProviderId};

use super::{
    ClientDbSourceIndexImport, ClientDbSourceIndexOwner, ClientDbSourceIndexPath,
    ClientDbSourceIndexQueryKey, ClientDbSourceIndexSelector, ClientDbSourceIndexSelectorId,
    ClientDbSourceIndexSelectorKind, ClientDbSourceIndexSelectorSymbol, ClientDbSourceIndexSource,
    ClientDbSourceIndexSourceBlobs,
};
use crate::{ClientDbActiveGenerationSourceBlobs, ClientDbSourceIndexGenerationSnapshot};

fn validate_overlay_membership(
    changed_owner_paths: &BTreeSet<String>,
    removed_owner_paths: &BTreeSet<String>,
    partial: &ClientDbSourceIndexImport,
) -> Result<(), String> {
    if !removed_owner_paths.is_disjoint(changed_owner_paths) {
        return Err("source-index owner cannot be both changed and tombstoned".to_string());
    }
    for owner in &partial.owners {
        if !changed_owner_paths.contains(owner.owner_path.as_str()) {
            return Err(format!(
                "incremental source-index import escaped changed membership: ownerPath={} changedOwnerPaths={changed_owner_paths:?}",
                owner.owner_path.as_str(),
            ));
        }
        if removed_owner_paths.contains(owner.owner_path.as_str()) {
            return Err(format!(
                "incremental source-index import contains tombstoned owner: ownerPath={}",
                owner.owner_path.as_str()
            ));
        }
    }
    Ok(())
}

fn base_owner(
    owner: &crate::ClientDbSourceIndexGenerationOwner,
) -> Result<ClientDbSourceIndexOwner, String> {
    Ok(ClientDbSourceIndexOwner {
        owner_path: ClientDbSourceIndexPath::new(owner.owner_path.clone()),
        language_id: owner.language_id.as_deref().map(LanguageId::new),
        provider_id: owner.provider_id.as_deref().map(ProviderId::new),
        source_kind: ClientDbSourceIndexSource::new(owner.source_kind.clone()),
        line_count: owner
            .line_count
            .map(u32::try_from)
            .transpose()
            .map_err(|_| format!("active owner line count is invalid: {}", owner.owner_path))?,
        query_keys: owner
            .query_keys
            .iter()
            .cloned()
            .map(ClientDbSourceIndexQueryKey::new)
            .collect(),
    })
}

fn base_selectors(
    owner: &crate::ClientDbSourceIndexGenerationOwner,
) -> Result<Vec<ClientDbSourceIndexSelector>, String> {
    let provider_id = ProviderId::new(owner.provider_id.as_deref().ok_or_else(|| {
        format!(
            "active selector owner has no provider: {}",
            owner.owner_path
        )
    })?);
    Ok(owner
        .selectors
        .iter()
        .map(|selector| ClientDbSourceIndexSelector {
            owner_path: ClientDbSourceIndexPath::new(owner.owner_path.clone()),
            provider_id: provider_id.clone(),
            selector_id: ClientDbSourceIndexSelectorId::new(selector.selector_id.clone()),
            symbol: selector
                .symbol
                .clone()
                .map(ClientDbSourceIndexSelectorSymbol::new),
            kind: selector
                .kind
                .clone()
                .map(ClientDbSourceIndexSelectorKind::new),
            source: ClientDbSourceIndexSource::new(selector.source.clone()),
            query_keys: selector
                .query_keys
                .iter()
                .cloned()
                .map(ClientDbSourceIndexQueryKey::new)
                .collect(),
            projection_record: selector.projection_record.clone(),
            derived_projections: selector.derived_projections.clone(),
        })
        .collect())
}

/// Derive one complete successor import without reopening unchanged owner bytes.
///
/// The returned packet is for canonical materialization. Durable Turso refresh
/// still receives the changed-owner import and applies it through a Merkle
/// overlay, so unchanged DB rows are cloned rather than rewritten.
pub fn overlay_active_source_index_import(
    active: &ClientDbSourceIndexGenerationSnapshot,
    active_blobs: &ClientDbActiveGenerationSourceBlobs,
    partial: &ClientDbSourceIndexImport,
    changed_owner_paths: &BTreeSet<String>,
    removed_owner_paths: &BTreeSet<String>,
) -> Result<ClientDbSourceIndexImport, String> {
    validate_overlay_membership(changed_owner_paths, removed_owner_paths, partial)?;
    if active.generation_id != active_blobs.generation_id {
        return Err(format!(
            "active source-index facts and bytes are from different generations: facts={} bytes={}",
            active.generation_id, active_blobs.generation_id
        ));
    }

    let mut file_hashes = active
        .file_hash_records
        .iter()
        .filter(|record| {
            !changed_owner_paths.contains(record.path.as_str())
                && !removed_owner_paths.contains(record.path.as_str())
                && !changed_owner_paths
                    .iter()
                    .chain(removed_owner_paths.iter())
                    .any(|owner_path| {
                        record.path == format!("@scope/selector-generation/{owner_path}")
                    })
        })
        .cloned()
        .map(|record| (record.path.clone(), record))
        .collect::<BTreeMap<_, _>>();
    for record in &partial.file_hashes {
        file_hashes.insert(record.path.clone(), record.clone());
    }

    let mut owners = Vec::new();
    let mut selectors = Vec::new();
    for owner in active.owners.iter().filter(|owner| {
        !changed_owner_paths.contains(owner.owner_path.as_str())
            && !removed_owner_paths.contains(owner.owner_path.as_str())
    }) {
        owners.push(base_owner(owner)?);
        selectors.extend(base_selectors(owner)?);
    }
    owners.extend(partial.owners.iter().cloned());
    selectors.extend(partial.selectors.iter().cloned());
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    selectors.sort_by(|left, right| left.selector_id.cmp(&right.selector_id));

    let mut relations = active
        .relations
        .iter()
        .filter(|relation| {
            !changed_owner_paths.contains(relation.owner_path.as_str())
                && !removed_owner_paths.contains(relation.owner_path.as_str())
        })
        .map(|relation| relation.relation.clone())
        .collect::<Vec<_>>();
    relations.extend(partial.relations.iter().cloned());
    relations.sort();
    relations.dedup();

    let mut source_blobs = active_blobs
        .owners
        .iter()
        .filter(|blob| {
            !changed_owner_paths.contains(blob.owner_path.as_str())
                && !removed_owner_paths.contains(blob.owner_path.as_str())
        })
        .map(|blob| {
            (
                ClientDbSourceIndexPath::new(blob.owner_path.clone()),
                blob.source_bytes.to_vec(),
            )
        })
        .collect::<Vec<_>>();
    source_blobs.extend(partial.source_blobs.iter().map(|(path, bytes)| {
        (
            ClientDbSourceIndexPath::new(path.to_string()),
            bytes.to_vec(),
        )
    }));

    Ok(ClientDbSourceIndexImport {
        generation_id: partial.generation_id.clone(),
        project_root: partial.project_root.clone(),
        schema_id: partial.schema_id.clone(),
        schema_version: partial.schema_version.clone(),
        file_hashes: file_hashes.into_values().collect(),
        source_blobs: ClientDbSourceIndexSourceBlobs::from_normalized(source_blobs),
        owners,
        selectors,
        relations,
    })
}
