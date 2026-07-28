use agent_semantic_content_identity::{
    CanonicalItemIdentityV1, CanonicalItemSelectorV1, ExactSelectorMaterializationProofV1,
    ExactSelectorProjectionModeV1,
};

pub(crate) struct MaterializationFixtureInput<'a> {
    pub language_id: &'a str,
    pub provider_id: &'a str,
    pub owner_path: &'a str,
    pub structural_selector: &'a str,
    pub item_kind: &'a str,
    pub item_name: &'a str,
    pub source: &'a [u8],
    pub source_byte_start: u64,
    pub source_byte_end: u64,
}

pub(crate) fn materialization_proof(
    input: MaterializationFixtureInput<'_>,
) -> ExactSelectorMaterializationProofV1 {
    let source_start =
        usize::try_from(input.source_byte_start).expect("fixture source start fits usize");
    let source_end = usize::try_from(input.source_byte_end).expect("fixture source end fits usize");
    let projection = input
        .source
        .get(source_start..source_end)
        .expect("fixture byte range belongs to source")
        .to_vec();
    let source_blob_digest = *blake3::hash(input.source).as_bytes();
    let owner_subtree_digest = owner_subtree_digest(input.owner_path, &source_blob_digest);
    let normalized_parser_facts = format!(
        "language={};kind={};name={};selector={}",
        input.language_id, input.item_kind, input.item_name, input.structural_selector
    );
    ExactSelectorMaterializationProofV1 {
        language_id: input.language_id.to_string(),
        provider_id: input.provider_id.to_string(),
        canonical_item_selector: CanonicalItemSelectorV1::new(
            CanonicalItemIdentityV1::new(input.language_id, input.item_kind, input.item_name),
            input.structural_selector,
        ),
        parser_identity_digest: domain_digest(
            b"asp.test.parser-identity.v1\0",
            input.provider_id.as_bytes(),
        ),
        query_pack_digest: domain_digest(
            b"asp.test.query-pack-identity.v1\0",
            input.language_id.as_bytes(),
        ),
        workspace_root_digest: owner_subtree_digest,
        owner_path: input.owner_path.to_string(),
        owner_subtree_digest,
        owner_inclusion_proof: Vec::new(),
        source_blob_digest,
        normalized_parser_facts_digest: *blake3::hash(normalized_parser_facts.as_bytes())
            .as_bytes(),
        structural_selector: input.structural_selector.to_string(),
        projection_mode: ExactSelectorProjectionModeV1::Code,
        source_byte_start: input.source_byte_start,
        source_byte_end: input.source_byte_end,
        projection_digest: *blake3::hash(&projection).as_bytes(),
        projection,
    }
}

fn owner_subtree_digest(owner_path: &str, source_blob_digest: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.exact-selector.owner-merkle-leaf.v1\0");
    hasher.update(&(owner_path.len() as u64).to_be_bytes());
    hasher.update(owner_path.as_bytes());
    hasher.update(source_blob_digest);
    *hasher.finalize().as_bytes()
}

fn domain_digest(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
    *hasher.finalize().as_bytes()
}
