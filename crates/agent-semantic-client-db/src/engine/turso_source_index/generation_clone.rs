pub(super) async fn clone_active_generation_rows(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    base_generation_id: &str,
    target_generation_id: &str,
) -> Result<(), String> {
    if base_generation_id == target_generation_id {
        return Err(
            "source-index overlay target must differ from its immutable base generation"
                .to_string(),
        );
    }
    for (surface, statement) in [
        (
            "owner",
            "INSERT INTO asp_source_index_owner_v1 (project_root, schema_id, schema_version, generation_id, file_hash, owner_path, language_id, provider_id, source_kind, line_count, query_keys_json, selector_facts_json, term_tokens_json, selector_count) SELECT project_root, schema_id, schema_version, ?5, file_hash, owner_path, language_id, provider_id, source_kind, line_count, query_keys_json, selector_facts_json, term_tokens_json, selector_count FROM asp_source_index_owner_v1 WHERE project_root = ?1 AND schema_id = ?2 AND schema_version = ?3 AND generation_id = ?4 ON CONFLICT DO NOTHING",
        ),
        (
            "selector",
            "INSERT INTO asp_source_index_selector_v1 (project_root, schema_id, schema_version, generation_id, owner_path, owner_content_digest, language_id, parser_identity_digest, query_pack_digest, item_kind, item_symbol, scopes_json, structural_selector) SELECT project_root, schema_id, schema_version, ?5, owner_path, owner_content_digest, language_id, parser_identity_digest, query_pack_digest, item_kind, item_symbol, scopes_json, structural_selector FROM asp_source_index_selector_v1 WHERE project_root = ?1 AND schema_id = ?2 AND schema_version = ?3 AND generation_id = ?4 ON CONFLICT DO NOTHING",
        ),
        (
            "token-owner",
            "INSERT INTO asp_source_index_token_owner_v1 (project_root, schema_id, schema_version, generation_id, token, owner_path) SELECT project_root, schema_id, schema_version, ?5, token, owner_path FROM asp_source_index_token_owner_v1 WHERE project_root = ?1 AND schema_id = ?2 AND schema_version = ?3 AND generation_id = ?4 ON CONFLICT DO NOTHING",
        ),
        (
            "relation",
            "INSERT INTO asp_source_index_relation_v1 (project_root, schema_id, schema_version, generation_id, owner_path, from_kind, from_id, relation_kind, to_kind, to_id) SELECT project_root, schema_id, schema_version, ?5, owner_path, from_kind, from_id, relation_kind, to_kind, to_id FROM asp_source_index_relation_v1 WHERE project_root = ?1 AND schema_id = ?2 AND schema_version = ?3 AND generation_id = ?4 ON CONFLICT DO NOTHING",
        ),
    ] {
        connection
            .execute(
                statement,
                (project_root, schema_id, schema_version, base_generation_id, target_generation_id),
            )
            .await
            .map_err(|error| format!("failed to clone active source-index {surface} rows: baseGenerationId={base_generation_id} targetGenerationId={target_generation_id} error={error}"))?;
    }
    Ok(())
}
