//! Reasoning tree for exact-selector generation fixtures.
//!
//! `core` owns the immutable binary layout and zero-copy lookup, while
//! `materialization_proof` owns the parser-produced Merkle evidence admitted
//! into that layout.

mod core;
mod materialization_proof;

pub(super) use core::DIGEST_LEN;

pub use core::{
    EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_ID, EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_VERSION,
    ExactSelectorGenerationFixtureErrorV1, ExactSelectorGenerationFixtureViewV1,
    ExactSelectorGenerationIdentityV1, ExactSelectorGenerationRecordV1,
    ExactSelectorGenerationRecordViewV1, ExactSelectorProjectionModeV1,
    build_exact_selector_generation_fixture_v1, fixture_digest_v1,
};
pub use materialization_proof::{
    ExactSelectorLanguageIdV1, ExactSelectorMaterializationProofErrorV1,
    ExactSelectorMaterializationProofV1, ExactSelectorMerkleProofSideV1,
    ExactSelectorMerkleProofStepV1, ExactSelectorOwnerPathV1, ExactSelectorProviderIdV1,
};
