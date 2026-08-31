use agent_semantic_search::{
    SearchGenerationSection, SearchGenerationSectionKind, SearchGenerationSectionRepresentation,
    ValidatedSearchGenerationSegment, encode_search_generation_segment,
};

fn sections() -> Vec<SearchGenerationSection> {
    [
        SearchGenerationSectionKind::GenerationEvidence,
        SearchGenerationSectionKind::ProjectResolutions,
        SearchGenerationSectionKind::OwnerDirectory,
        SearchGenerationSectionKind::OwnerBytes,
        SearchGenerationSectionKind::SelectorIndex,
        SearchGenerationSectionKind::GraphRelations,
        SearchGenerationSectionKind::MerkleOwnerIndex,
    ]
    .into_iter()
    .enumerate()
    .map(|(position, kind)| SearchGenerationSection {
        kind,
        representation: match kind {
            SearchGenerationSectionKind::GenerationEvidence
            | SearchGenerationSectionKind::ProjectResolutions => {
                SearchGenerationSectionRepresentation::Utf8Json as u8
            }
            SearchGenerationSectionKind::OwnerBytes => {
                SearchGenerationSectionRepresentation::OpaqueBytes as u8
            }
            _ => SearchGenerationSectionRepresentation::SortedOffsetTable as u8,
        },
        record_count: 1,
        bytes: vec![position as u8; position + 1],
    })
    .collect()
}

#[test]
fn v3_search_generation_segment_is_zero_copy_addressable() {
    let bytes = encode_search_generation_segment(7, sections()).expect("encode v3 segment");
    let segment = ValidatedSearchGenerationSegment::parse(&bytes).expect("validate v3 segment");
    assert_eq!(segment.epoch(), 7);
    let (owner_bytes, representation, records) =
        segment.section(SearchGenerationSectionKind::OwnerBytes);
    assert_eq!(owner_bytes, &[3, 3, 3, 3]);
    assert_eq!(
        representation,
        SearchGenerationSectionRepresentation::OpaqueBytes as u8
    );
    assert_eq!(records, 1);
}

#[test]
fn corrupt_or_truncated_v3_search_generation_is_rejected() {
    let bytes = encode_search_generation_segment(1, sections()).expect("encode v3 segment");
    assert!(ValidatedSearchGenerationSegment::parse(&bytes[..bytes.len() - 1]).is_err());
    let mut corrupt = bytes;
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    // Opening validates the fixed directory, while record payloads are validated
    // only when touched. Corrupt the directory commitment for the cold-open gate.
    corrupt[24] ^= 0x01;
    assert!(ValidatedSearchGenerationSegment::parse(&corrupt).is_err());
}

#[test]
fn missing_or_duplicate_v3_sections_are_rejected_before_publication() {
    let mut missing = sections();
    missing.pop();
    assert!(encode_search_generation_segment(1, missing).is_err());

    let mut duplicate = sections();
    duplicate[1].kind = SearchGenerationSectionKind::GenerationEvidence;
    assert!(encode_search_generation_segment(1, duplicate).is_err());
}
