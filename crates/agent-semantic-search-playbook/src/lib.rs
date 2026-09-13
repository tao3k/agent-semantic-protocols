// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public facade for the lightweight Search Playbook V1 contract.

mod playbook;

pub use playbook::{
    GraphNativeBlock, ProducerNativeBlock, ProgressiveSearchPlaybookError,
    ProgressiveSearchPlaybookRequest, SearchPlaybookClauseAxis, SearchPlaybookClauseRef,
    SearchPlaybookProducerDeclaration, parse_progressive_search_playbook_args,
    parse_search_playbook_producer_declaration,
};
