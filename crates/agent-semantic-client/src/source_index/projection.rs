//! Generation-time provider projection into canonical exact-selector records.

use std::collections::BTreeMap;
use std::path::Path;

use agent_semantic_client_core::{ProviderRegistrySnapshot, ResolvedProvider};
use agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind;
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
    projection_evidence_context::ProjectionEvidenceContext,
    semantic_projection::SemanticProjection,
    workspace_merkle_v1::WorkspacePathMerkleTreeV1,
};
use agent_semantic_provider_transport::ProviderRuntimeActorClient;
use agent_semantic_provider_transport::projection_batch::{
    ProviderProjectedItem, ProviderProjectedOwner, ProviderProjectionBatchRequest,
    ProviderProjectionOwner, provider_projection_batch_ranges,
};

enum ProviderProjectionExecutor<'a> {
    Resident(&'a ProviderRuntimeActorClient),
    RuntimeService(
        &'a agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    ),
}

pub(super) async fn project_generation_with_resident_runtime(
    runtime: &ProviderRuntimeActorClient,
    project_root: &Path,
    workspace_identity: &str,
    registry: &ProviderRegistrySnapshot,
    files: &[ClientDbSourceIndexScopeFile],
    source_blobs: &ClientDbSourceIndexSourceBlobs,
) -> Result<Vec<ClientDbSourceIndexScopeFile>, String> {
    project_generation_with_executor(
        ProviderProjectionExecutor::Resident(runtime),
        project_root,
        workspace_identity,
        registry,
        files,
        source_blobs,
    )
    .await
}

pub(super) async fn project_generation_with_runtime_service(
    runtime: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    project_root: &Path,
    workspace_identity: &str,
    registry: &ProviderRegistrySnapshot,
    files: &[ClientDbSourceIndexScopeFile],
    source_blobs: &ClientDbSourceIndexSourceBlobs,
) -> Result<Vec<ClientDbSourceIndexScopeFile>, String> {
    project_generation_with_executor(
        ProviderProjectionExecutor::RuntimeService(runtime),
        project_root,
        workspace_identity,
        registry,
        files,
        source_blobs,
    )
    .await
}

async fn project_generation_with_executor(
    executor: ProviderProjectionExecutor<'_>,
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
        .filter(|provider| provider.runtime_operation("projection-batch").is_some())
    {
        project_provider(
            &executor,
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
    executor: &ProviderProjectionExecutor<'_>,
    project_root: &Path,
    workspace_identity: &str,
    provider: &ResolvedProvider,
    tree: &WorkspacePathMerkleTreeV1,
    source_blobs: &ClientDbSourceIndexSourceBlobs,
    files: &mut [ClientDbSourceIndexScopeFile],
) -> Result<(), String> {
    let operation = provider
        .runtime_operation("projection-batch")
        .ok_or_else(|| {
            format!(
                "provider lacks projection-batch runtime operation: providerId={}",
                provider.provider_id
            )
        })?;
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
        let response = match executor {
            ProviderProjectionExecutor::Resident(runtime) => {
                let encoded = request.encode().map_err(|error| error.to_string())?;
                let response = runtime.request(&operation.operation, encoded).await?;
                agent_semantic_provider_transport::projection_batch::ProviderProjectionBatchResponse::decode_for(
                    &request,
                    &response,
                )
                .map_err(|error| error.to_string())?
            }
            ProviderProjectionExecutor::RuntimeService(runtime) => {
                runtime
                    .provider_runtime(
                        project_root.to_path_buf(),
                        provider.language_id.as_str().to_owned(),
                    )
                    .await?;
                runtime
                    .provider_runtime_await_ready(
                        project_root.to_path_buf(),
                        provider.language_id.as_str().to_owned(),
                    )
                    .await?;
                let encoded = request.encode().map_err(|error| error.to_string())?;
                let response = runtime
                    .provider_operation(
                        project_root.to_path_buf(),
                        provider.language_id.as_str().to_owned(),
                        operation.operation.clone(),
                        encoded,
                    )
                    .await?;
                agent_semantic_provider_transport::projection_batch::ProviderProjectionBatchResponse::decode_for(
                    &request,
                    &response,
                )
                .map_err(|error| error.to_string())?
            }
        };
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

fn encode_semantic_projection(
    projection_kind: ExactProjectionKind,
    language_id: &str,
    provider_id: &str,
    selector: &str,
    evidence_context_ref: &str,
    payload: &serde_json::Value,
) -> Result<Vec<u8>, String> {
    match projection_kind {
        ExactProjectionKind::CallableSkeleton => {
            let payload = serde_json::from_value::<
                agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload,
            >(payload.clone())
            .map_err(|error| {
                format!("decode provider callable-skeleton projection payload: {error}")
            })?;
            let envelope = SemanticProjection::new(
                projection_kind.as_str(),
                language_id,
                provider_id,
                selector,
                evidence_context_ref,
                "agent.semantic-protocols.callable-skeleton",
                payload,
            )
            .map_err(|error| format!("build callable-skeleton projection envelope: {error}"))?;
            serde_json::to_vec(&envelope)
                .map_err(|error| format!("encode callable-skeleton projection envelope: {error}"))
        }
        ExactProjectionKind::Source => {
            let envelope = SemanticProjection::new(
                projection_kind.as_str(),
                language_id,
                provider_id,
                selector,
                evidence_context_ref,
                "agent.semantic-protocols.exact-source",
                payload.clone(),
            )
            .map_err(|error| format!("build source projection envelope: {error}"))?;
            serde_json::to_vec(&envelope)
                .map_err(|error| format!("encode source projection envelope: {error}"))
        }
    }
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
            let record_structural_selector = record.proof.structural_selector().to_owned();
            if record_structural_selector != item.selector {
                return Err("provider projection record selector drift".to_owned());
            }
            let evidence_context = ProjectionEvidenceContext::from_exact_selector_proof(
                provider.provider_id.as_str(),
                &record.proof,
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
                        let projection_kind = agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
                            projection.projection_kind.as_str(),
                        )
                        .map_err(|error| format!("decode provider projection kind: {error}"))?;
                        let bytes = encode_semantic_projection(
                            projection_kind,
                            provider.language_id.as_str(),
                            provider.provider_id.as_str(),
                            item.selector.as_str(),
                            evidence_context.evidence_context_ref.as_str(),
                            &projection.payload,
                        )?;
                        Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
                            projection_kind,
                            bytes,
                            evidence_context: Some(evidence_context.clone()),
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

fn relative_owner_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
