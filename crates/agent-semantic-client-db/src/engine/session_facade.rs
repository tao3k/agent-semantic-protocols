//! DB Engine read and write session facade methods.

use std::path::Path;

use agent_semantic_client_core::{
    CacheExportMethod, ClientCacheFileHash, ClientCacheGeneration, LanguageId, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion,
};

use crate::source_index::{
    ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexStats,
};
use crate::structural_index::parse_structural_index_packet_import;
use crate::types::{
    ClientDbArtifactEvent, ClientDbGenerationHit, ClientDbProviderCommandSelection,
    ClientDbSyntaxQueryLookup, ClientDbSyntaxQueryReplay,
};

use super::facade::{
    ClientDbEngineReadSession, ClientDbEngineWriteSession, block_on_db_engine_async,
};
use super::source_index_facade::persist_structural_index_read_model_at_path;
use super::turso_artifact::{lookup_turso_artifact_events, upsert_turso_artifact_events};
use super::turso_bootstrap::bootstrap_turso_client_db;
use super::turso_cache::{
    clear_turso_cache_generations, invalidate_turso_cache_generations_for_project,
    lookup_recent_turso_cache_generations, prune_turso_cache_generations_to_manifest,
    upsert_turso_cache_generations,
};
use super::turso_provider_command::{
    lookup_turso_provider_command_selections, replace_turso_provider_command_selections,
};
use super::turso_source_index::{
    latest_turso_source_index_file_hashes, latest_turso_source_index_scope_files,
    latest_turso_source_index_stats, lookup_reusable_turso_source_index_generation,
    refresh_turso_source_index_import,
};
use super::turso_syntax::{
    flush_turso_syntax_query_replay, lookup_turso_syntax_query_replay,
    upsert_turso_syntax_query_replay,
};

impl ClientDbEngineReadSession {
    /// Inspect the opened DB Engine session without exposing its concrete backend type.
    pub fn inspect(&self) -> Result<crate::ClientDbReport, String> {
        Ok(super::facade_turso_report::turso_client_db_report(
            &self.turso_db_path,
        ))
    }

    /// Return matching generation metadata using this already opened DB Engine session.
    pub fn lookup_generation_request(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        export_method: &CacheExportMethod,
        request_fingerprint: Option<String>,
    ) -> Result<Option<ClientDbGenerationHit>, String> {
        let turso_db_path = self.turso_db_path.clone();
        let language_id = language_id.clone();
        let provider_id = provider_id.clone();
        let project_root = project_root.to_path_buf();
        let export_method = export_method.clone();
        let turso_hits = block_on_db_engine_async(async move {
            lookup_recent_turso_cache_generations(
                &turso_db_path,
                &language_id,
                &provider_id,
                &project_root,
                &export_method,
                request_fingerprint.as_deref(),
                1,
            )
            .await
        })?;
        Ok(turso_hits.into_iter().next())
    }

    /// Return recent matching generation metadata using this already opened DB Engine session.
    pub fn lookup_recent_generations_request(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        export_method: &CacheExportMethod,
        request_fingerprint: Option<String>,
        limit: u32,
    ) -> Result<Vec<ClientDbGenerationHit>, String> {
        let turso_db_path = self.turso_db_path.clone();
        let language_id = language_id.clone();
        let provider_id = provider_id.clone();
        let project_root = project_root.to_path_buf();
        let export_method = export_method.clone();
        block_on_db_engine_async(async move {
            lookup_recent_turso_cache_generations(
                &turso_db_path,
                &language_id,
                &provider_id,
                &project_root,
                &export_method,
                request_fingerprint.as_deref(),
                limit,
            )
            .await
        })
    }

    /// Return syntax query replay rows using this already opened DB Engine session.
    pub fn lookup_syntax_query_replay_request(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        query_ast_fingerprint: String,
        selector: Option<String>,
    ) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
        let lookup = ClientDbSyntaxQueryLookup {
            db_path: self.turso_db_path.clone(),
            language_id: language_id.clone(),
            provider_id: provider_id.clone(),
            project_root: project_root.to_path_buf(),
            query_ast_fingerprint,
            selector,
        };
        block_on_db_engine_async(async move {
            lookup_turso_syntax_query_replay(&lookup.db_path, &lookup).await
        })
    }

    /// Return graph-turbo artifact events using this already opened DB Engine session.
    pub fn lookup_artifact_events(
        &self,
        since_timestamp_ms: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEvent>, String> {
        let db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            lookup_turso_artifact_events(&db_path, since_timestamp_ms, limit).await
        })
    }

    /// Return cached provider command selections through this DB Engine session.
    pub fn lookup_provider_command_selections(
        &self,
        project_root: &Path,
        context_fingerprint: &str,
    ) -> Result<Option<Vec<ClientDbProviderCommandSelection>>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let context_fingerprint = context_fingerprint.to_string();
        block_on_db_engine_async(async move {
            lookup_turso_provider_command_selections(&db_path, &project_root, &context_fingerprint)
                .await
        })
    }
}

impl ClientDbEngineWriteSession {
    /// Inspect the opened DB Engine session without exposing its concrete backend type.
    pub fn inspect(&self) -> Result<crate::ClientDbReport, String> {
        Ok(super::facade_turso_report::turso_client_db_report(
            &self.turso_db_path,
        ))
    }

    /// Import one cache manifest through the DB Engine control adapter.
    pub fn import_manifest(
        &mut self,
        manifest: &agent_semantic_client_core::ClientCacheManifest,
    ) -> Result<(), String> {
        let turso_db_path = self.turso_db_path.clone();
        let manifest = manifest.clone();
        block_on_db_engine_async(async move {
            upsert_turso_cache_generations(&turso_db_path, &manifest)
                .await
                .map(|_| ())
        })
    }

    /// Synchronize the control adapter's generation universe before manifest writeback import.
    pub fn sync_cache_generations_for_manifest_writeback(
        &mut self,
        manifest: &agent_semantic_client_core::ClientCacheManifest,
        status: &agent_semantic_client_core::CacheManifestStatus,
    ) -> Result<(), String> {
        if matches!(
            status,
            agent_semantic_client_core::CacheManifestStatus::Unavailable
        ) {
            return Err("cache manifest is unavailable for DB Engine writeback sync".to_string());
        }
        let turso_db_path = self.turso_db_path.clone();
        let status = status.clone();
        let manifest = manifest.clone();
        block_on_db_engine_async(async move {
            match status {
                agent_semantic_client_core::CacheManifestStatus::Missing
                | agent_semantic_client_core::CacheManifestStatus::Invalid => {
                    clear_turso_cache_generations(&turso_db_path).await
                }
                agent_semantic_client_core::CacheManifestStatus::Present => {
                    prune_turso_cache_generations_to_manifest(&turso_db_path, &manifest).await
                }
                agent_semantic_client_core::CacheManifestStatus::Unavailable => Ok(()),
            }
        })
    }

    /// Upsert artifact events through the DB Engine control adapter.
    pub fn upsert_artifact_events(
        &mut self,
        events: &[ClientDbArtifactEvent],
    ) -> Result<u32, String> {
        let db_path = self.turso_db_path.clone();
        let events = events.to_vec();
        block_on_db_engine_async(
            async move { upsert_turso_artifact_events(&db_path, &events).await },
        )
    }

    /// Replace cached provider command selections through this DB Engine session.
    pub fn replace_provider_command_selections(
        &mut self,
        project_root: &Path,
        context_fingerprint: &str,
        selections: &[ClientDbProviderCommandSelection],
    ) -> Result<(), String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let context_fingerprint = context_fingerprint.to_string();
        let selections = selections.to_vec();
        block_on_db_engine_async(async move {
            replace_turso_provider_command_selections(
                &db_path,
                &project_root,
                &context_fingerprint,
                &selections,
            )
            .await
        })
    }

    /// Return file hash evidence from the latest source-index generation.
    pub fn latest_source_index_file_hashes(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let schema_id = schema_id.clone();
        let schema_version = schema_version.clone();
        block_on_db_engine_async(async move {
            latest_turso_source_index_file_hashes(
                &db_path,
                &project_root,
                &schema_id,
                &schema_version,
            )
            .await
        })
    }

    /// Return file-scoped source-index inputs from the latest generation.
    pub fn latest_source_index_scope_files(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let schema_id = schema_id.clone();
        let schema_version = schema_version.clone();
        block_on_db_engine_async(async move {
            latest_turso_source_index_scope_files(
                &db_path,
                &project_root,
                &schema_id,
                &schema_version,
            )
            .await
        })
    }

    /// Return stats from the latest source-index generation.
    pub fn latest_source_index_stats(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<ClientDbSourceIndexStats>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let schema_id = schema_id.clone();
        let schema_version = schema_version.clone();
        block_on_db_engine_async(async move {
            latest_turso_source_index_stats(&db_path, &project_root, &schema_id, &schema_version)
                .await
        })
    }

    /// Return reusable source-index stats when the latest evidence is unchanged.
    pub fn reusable_source_index_generation(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
        file_hashes: &[ClientCacheFileHash],
    ) -> Result<Option<ClientDbSourceIndexStats>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let schema_id = schema_id.clone();
        let schema_version = schema_version.clone();
        let file_hashes = file_hashes.to_vec();
        block_on_db_engine_async(async move {
            lookup_reusable_turso_source_index_generation(
                &db_path,
                &project_root,
                &schema_id,
                &schema_version,
                &file_hashes,
            )
            .await
        })
    }

    /// Apply a source-index import through this DB Engine session.
    pub fn refresh_source_index_import(
        &mut self,
        request: ClientDbSourceIndexRefreshRequest,
    ) -> Result<ClientDbSourceIndexRefreshReport, String> {
        let db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            refresh_turso_source_index_import(&db_path, request).await
        })
    }

    /// Import one semantic tree-sitter query packet through the DB Engine control adapter.
    pub fn import_semantic_tree_sitter_query_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        let turso_db_path = self.turso_db_path.clone();
        let generation = generation.clone();
        let packet_bytes = packet_bytes.to_vec();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&turso_db_path).await?;
            upsert_turso_syntax_query_replay(&turso_db_path, &generation, &packet_bytes).await
        })
    }

    /// Import one structural-index refresh artifact through the DB Engine control adapter.
    pub fn import_semantic_structural_index_refresh_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Result<(), String> {
        let import = parse_structural_index_packet_import(generation, packet_bytes)?;
        let db_path = self.turso_db_path.clone();
        let source_snapshot = source_snapshot.clone();
        block_on_db_engine_async(async move {
            persist_structural_index_read_model_at_path(&db_path, &import, &source_snapshot)
                .await
                .map(|_| ())
        })
    }

    /// Flush syntax query replay rows through this already opened DB Engine session.
    pub fn flush_syntax_query_rows(&mut self) -> Result<u32, String> {
        let turso_db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&turso_db_path).await?;
            flush_turso_syntax_query_replay(&turso_db_path).await
        })
    }

    /// Invalidate local generation rows for one project through this DB Engine session.
    pub fn invalidate_generations_for_project(
        &mut self,
        project_root: impl AsRef<Path>,
    ) -> Result<u32, String> {
        let project_root = project_root.as_ref().to_path_buf();
        let turso_db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&turso_db_path).await?;
            invalidate_turso_cache_generations_for_project(&turso_db_path, &project_root).await
        })
    }
}
