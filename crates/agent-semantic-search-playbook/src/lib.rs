// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public facade for the pure typed Search Playbook V1 IR contract.

mod model;
mod native_rg;
mod playbook;
mod tantivy_query;
mod topology_owner;

pub use model::{
    GraphNativeBlock, ProducerNativeBlock, ProgressiveSearchPlaybookError,
    ProgressiveSearchPlaybookRequest, SEARCH_PLAYBOOK_COMPOSITION_ABI_ID,
    SEARCH_PLAYBOOK_COMPOSITION_ABI_VERSION, SEARCH_PLAYBOOK_COMPOSITION_OPERATORS,
    SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH, SEARCH_PLAYBOOK_MAX_COMPOSITION_NODES,
    SEARCH_PLAYBOOK_MAX_STATIC_WORK, SearchPlaybookClauseAxis, SearchPlaybookClauseRef,
    SearchPlaybookComposition, SearchPlaybookCompositionMetrics, SearchPlaybookLeaf,
    SearchPlaybookNormalizedComposition, SearchPlaybookPlanInput,
    SearchPlaybookProducerDeclaration, TopologyOwnerMembershipBlock,
    search_playbook_composition_abi_digest,
};
pub use native_rg::{
    NativeRgArgvAnalysis, NativeRgDiagnostic, NativeRgDiagnosticKind, NativeRgOptionOccurrence,
    NativeRgOutputAttribution, NativeRgValueOccurrence, analyze_native_rg_argv,
};
pub use playbook::admit_search_playbook_plan;
pub use tantivy_query::{
    TantivyQueryAnalysis, TantivyQueryDiagnostic, TantivyQueryMetrics, analyze_tantivy_query,
};
