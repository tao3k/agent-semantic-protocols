// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public facade for the lightweight Search Playbook V1 contract.

mod model;
mod playbook;

pub use model::{
    GraphNativeBlock, ProducerNativeBlock, ProgressiveSearchPlaybookError,
    ProgressiveSearchPlaybookRequest, SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH,
    SEARCH_PLAYBOOK_MAX_COMPOSITION_NODES, SEARCH_PLAYBOOK_MAX_STATIC_WORK,
    SearchPlaybookClauseAxis, SearchPlaybookClauseRef, SearchPlaybookComposition,
    SearchPlaybookCompositionMetrics, SearchPlaybookLeaf, SearchPlaybookNormalizedComposition,
    SearchPlaybookProducerDeclaration,
};
pub use playbook::{
    parse_progressive_search_playbook_args, parse_query_playbook_producer_declaration,
    parse_search_playbook_producer_declaration,
};
