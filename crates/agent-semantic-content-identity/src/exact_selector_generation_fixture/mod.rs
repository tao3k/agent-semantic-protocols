// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Reasoning tree for exact-selector generation fixtures.
//!
//! `core` owns the immutable binary layout and zero-copy lookup, while
//! `materialization_proof` owns the parser-produced Merkle evidence admitted
//! into that layout.

mod core;
mod materialization_proof;

pub(super) use core::DIGEST_LEN;

pub use core::EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_ID;
pub use core::EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_VERSION;
pub use core::ExactSelectorGenerationFixtureErrorV1;
pub use core::ExactSelectorGenerationFixtureViewV1;
pub use core::ExactSelectorGenerationIdentityV1;
pub use core::ExactSelectorGenerationRecordV1;
pub use core::ExactSelectorGenerationRecordViewV1;
pub use core::ExactSelectorProjectionModeV1;
pub use core::build_exact_selector_generation_fixture_v1;
pub use core::fixture_digest_v1;
pub use materialization_proof::ExactSelectorLanguageIdV1;
pub use materialization_proof::ExactSelectorMaterializationProofErrorV1;
pub use materialization_proof::ExactSelectorMaterializationProofV1;
pub use materialization_proof::ExactSelectorMerkleProofSideV1;
pub use materialization_proof::ExactSelectorMerkleProofStepV1;
pub use materialization_proof::ExactSelectorOwnerPathV1;
pub use materialization_proof::ExactSelectorProviderIdV1;
