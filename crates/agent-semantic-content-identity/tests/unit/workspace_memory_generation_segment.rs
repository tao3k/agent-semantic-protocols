use agent_semantic_content_identity::workspace_memory_generation_segment::{
    WORKSPACE_MEMORY_GENERATION_SEGMENT_MAGIC, has_current_workspace_memory_generation_contract,
};

#[test]
fn current_relation_generation_segment_is_admitted() {
    assert!(has_current_workspace_memory_generation_contract(
        WORKSPACE_MEMORY_GENERATION_SEGMENT_MAGIC
    ));
}

#[test]
fn legacy_generation_segment_is_rejected_before_payload_decode() {
    assert!(!has_current_workspace_memory_generation_contract(
        b"ASPWSGENERATION1legacy-payload"
    ));
}
