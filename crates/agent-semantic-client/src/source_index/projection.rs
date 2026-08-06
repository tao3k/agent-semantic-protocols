//! Generation-time provider projection into canonical exact-selector records.

use std::collections::BTreeMap;
use std::path::Path;

use agent_semantic_client_core::{ProviderRegistrySnapshot, ResolvedProvider};
use agent_semantic_client_db::{
    ClientDbSourceIndexPath, ClientDbSourceIndexProjectionCoverage, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector, ClientDbSourceIndexSelectorId,
    ClientDbSourceIndexSelectorKind, ClientDbSourceIndexSelectorSymbol, ClientDbSourceIndexSource,
    ClientDbSourceIndexSourceBlobs,
};
use agent_semantic_content_identity::{
    exact_selector_merkle::{ExactProjectionModeV1, blake3_content_digest_v1},
    exact_selector_projection_packet::{
        ExactSelectorProjectionPacketV1Input, ProjectionPacketExecutionCommandDigestV1,
        ProjectionPacketLanguageIdV1, ProjectionPacketOwnerPathV1, ProjectionPacketProviderIdV1,
        ProjectionPacketSemanticRegistryDigestV1, ProjectionPacketStructuralSelectorV1,
        build_exact_selector_projection_packet_v1, derive_parser_identity_digest_v1,
        derive_query_pack_identity_digest_v1,
    },
    workspace_merkle_v1::WorkspacePathMerkleTreeV1,
};
use agent_semantic_provider_transport::projection_batch::{
    ProviderProjectedItem, ProviderProjectedOwner, ProviderProjectionBatchRequest,
    ProviderProjectionOwner, provider_projection_batch_ranges, run_provider_projection_batch,
};

pub(super) async fn project_generation(
    project_root: &Path,
    workspace_identity: &str,
    registry: &ProviderRegistrySnapshot,
    files: &[ClientDbSourceIndexScopeFile],
    source_blobs: &ClientDbSourceIndexSourceBlobs,
) -> Result<Vec<ClientDbSourceIndexScopeFile>, String> {
    let tree = WorkspacePathMerkleTreeV1::from_file_digests(
        source_blobs
            .iter()
            .map(|(owner_path, bytes)| (owner_path.to_owned(), blake3_content_digest_v1(bytes))),
    )
    .map_err(|error| format!("build exact-selector workspace Merkle tree: {error}"))?;
    let mut projected = files.to_vec();
    for provider in registry
        .providers
        .iter()
        .filter(|provider| provider.language_projection.is_some())
    {
        project_provider(
            project_root,
            workspace_identity,
            provider,
            &tree,
            source_blobs,
            &mut projected,
        )
        .await?;
    }
    Ok(projected)
}

async fn project_provider(
    project_root: &Path,
    workspace_identity: &str,
    provider: &ResolvedProvider,
    tree: &WorkspacePathMerkleTreeV1,
    source_blobs: &ClientDbSourceIndexSourceBlobs,
    files: &mut [ClientDbSourceIndexScopeFile],
) -> Result<(), String> {
    let descriptor = provider
        .language_projection
        .as_ref()
        .expect("projection-capable providers were filtered by the caller");
    let owner_indexes = files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.provider_id == provider.provider_id)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if owner_indexes.is_empty() {
        return Ok(());
    }
    let parser_identity_digest = derive_parser_identity_digest_v1(
        &ProjectionPacketProviderIdV1::from(provider.provider_id.as_str()),
        &ProjectionPacketExecutionCommandDigestV1::from(provider.execution_command_digest.as_str()),
        &ProjectionPacketSemanticRegistryDigestV1::from(provider.manifest_digest.as_str()),
    );
    let query_pack_json = serde_json::to_vec(&provider.query_pack_descriptor)
        .map_err(|error| format!("encode provider query-pack identity: {error}"))?;
    let query_pack_digest = derive_query_pack_identity_digest_v1(&query_pack_json);
    let command = provider_command_argv(provider)?;
    let owner_sizes = owner_indexes
        .iter()
        .map(|index| {
            let owner_path = relative_owner_path(project_root, &files[*index].path);
            source_blobs
                .get(&ClientDbSourceIndexPath::new(&owner_path))
                .map(|source| source.len())
                .ok_or_else(|| format!("projection source bytes are missing: {owner_path}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    for range in provider_projection_batch_ranges(&owner_sizes) {
        let batch_indexes = &owner_indexes[range];
        let owners = batch_indexes
            .iter()
            .map(|index| {
                let owner_path = relative_owner_path(project_root, &files[*index].path);
                let source = source_blobs
                    .get(&ClientDbSourceIndexPath::new(&owner_path))
                    .ok_or_else(|| format!("projection source bytes are missing: {owner_path}"))?;
                let source_leaf_digest = tree.source_blob_digest(&owner_path).ok_or_else(|| {
                    format!("projection owner is absent from Merkle tree: {owner_path}")
                })?;
                Ok(ProviderProjectionOwner {
                    owner_path,
                    source_leaf_digest: source_leaf_digest.as_str().to_owned(),
                    source_bytes: source.to_vec(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let request = ProviderProjectionBatchRequest {
            language_id: provider.language_id.as_str().to_owned(),
            provider_id: provider.provider_id.as_str().to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            generation_root_digest: tree.root_digest().as_str().to_owned(),
            parser_identity_digest: parser_identity_digest.as_str().to_owned(),
            query_pack_digest: query_pack_digest.as_str().to_owned(),
            base_generation_root_digest: None,
            owners,
        };
        let response = run_provider_projection_batch(
            &command,
            descriptor.command_binding(),
            project_root,
            &request,
        )
        .await
        .map_err(|error| error.to_string())?;
        let response_by_owner = response
            .owners
            .into_iter()
            .map(|owner| (owner.owner_path.clone(), owner))
            .collect::<BTreeMap<_, _>>();
        for index in batch_indexes {
            let file = &mut files[*index];
            let owner_path = relative_owner_path(project_root, &file.path);
            let projected_owner = response_by_owner.get(&owner_path).ok_or_else(|| {
                format!("projection response omitted admitted owner: {owner_path}")
            })?;
            let source = source_blobs
                .get(&ClientDbSourceIndexPath::new(&owner_path))
                .ok_or_else(|| format!("projection source bytes are missing: {owner_path}"))?;
            file.selector_receipts = selector_receipts(
                provider,
                tree,
                source,
                projected_owner,
                &parser_identity_digest,
                &query_pack_digest,
            )?;
            file.relations = projected_owner.relations.clone();
            file.projection_coverage = ClientDbSourceIndexProjectionCoverage::Complete;
        }
    }
    Ok(())
}

fn selector_receipts(
    provider: &ResolvedProvider,
    tree: &WorkspacePathMerkleTreeV1,
    source: &[u8],
    owner: &ProviderProjectedOwner,
    parser_identity_digest: &agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1,
    query_pack_digest: &agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1,
) -> Result<Vec<ClientDbSourceIndexSelector>, String> {
    let owner_inclusion_proof = tree.inclusion_proof(&owner.owner_path).ok_or_else(|| {
        format!(
            "projection owner is absent from Merkle tree: {}",
            owner.owner_path
        )
    })?;
    owner
        .items
        .iter()
        .map(|item| {
            let canonical = agent_semantic_content_identity::CanonicalItemSelector::parse(
                item.selector.as_str(),
            )
            .map_err(|error| format!("parse provider canonical selector: {error}"))?;
            let projection = source
                .get(item.source_byte_start..item.source_byte_end)
                .ok_or_else(|| {
                    format!(
                        "provider projection range is outside owner: {}",
                        item.selector
                    )
                })?;
            let normalized_facts = normalized_item_parser_facts(item)?;
            let language_id = ProjectionPacketLanguageIdV1::from(provider.language_id.as_str());
            let provider_id = ProjectionPacketProviderIdV1::from(provider.provider_id.as_str());
            let owner_path = ProjectionPacketOwnerPathV1::from(owner.owner_path.as_str());
            let structural_selector =
                ProjectionPacketStructuralSelectorV1::from(item.selector.as_str());
            let record =
                build_exact_selector_projection_packet_v1(ExactSelectorProjectionPacketV1Input {
                    source_byte_start: item.source_byte_start as u64,
                    source_byte_end: item.source_byte_end as u64,
                    language_id: &language_id,
                    provider_id: &provider_id,
                    canonical_item_selector: canonical,
                    parser_identity_digest,
                    query_pack_digest,
                    owner_path: &owner_path,
                    structural_selector: &structural_selector,
                    projection_mode: ExactProjectionModeV1::Code,
                    source,
                    normalized_parser_facts: &normalized_facts,
                    projection,
                })
                .enrich_projection_record_with_owner_inclusion_proof(
                    tree,
                    &owner_inclusion_proof,
                )
                .map_err(|error| format!("enrich provider projection record: {error:?}"))?;
            let mut query_keys = vec![ClientDbSourceIndexQueryKey::from(item.name.as_str())];
            query_keys.extend(
                item.identity
                    .scopes
                    .iter()
                    .map(|scope| ClientDbSourceIndexQueryKey::from(scope.symbol.as_str())),
            );
            Ok(ClientDbSourceIndexSelector {
                owner_path: ClientDbSourceIndexPath::new(&owner.owner_path),
                provider_id: provider.provider_id.clone(),
                selector_id: ClientDbSourceIndexSelectorId::from(item.selector.as_str()),
                symbol: Some(ClientDbSourceIndexSelectorSymbol::from(item.name.as_str())),
                kind: Some(ClientDbSourceIndexSelectorKind::from(item.kind.as_str())),
                source: ClientDbSourceIndexSource::from(provider.provider_id.as_str()),
                query_keys,
                projection_record: record,
                derived_projections: item
                    .projections
                    .iter()
                    .map(|projection| {
                        serde_json::to_vec(&projection.payload)
                            .map(|bytes| {
                                agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
                                    projection_kind: projection.projection_kind.clone(),
                                    bytes,
                                }
                            })
                            .map_err(|error| {
                                format!("encode provider derived projection payload: {error}")
                            })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            })
        })
        .collect()
}

fn normalized_item_parser_facts(item: &ProviderProjectedItem) -> Result<Vec<u8>, String> {
    serde_json::to_vec(item).map_err(|error| format!("encode provider item parser facts: {error}"))
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_projection_memory.rs"]
mod tests;

fn provider_command_argv(provider: &ResolvedProvider) -> Result<Vec<String>, String> {
    provider
        .runtime_command_argv
        .as_ref()
        .filter(|argv| !argv.is_empty())
        .cloned()
        .or_else(|| {
            (!provider.provider_command_prefix.is_empty())
                .then(|| provider.provider_command_prefix.clone())
        })
        .ok_or_else(|| {
            format!(
                "projection provider command is unavailable: languageId={} providerId={}",
                provider.language_id, provider.provider_id
            )
        })
}

fn relative_owner_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
