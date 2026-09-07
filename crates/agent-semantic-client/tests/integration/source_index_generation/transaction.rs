// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::server_source_index::PublishedSourceIndexGenerationV1;
use agent_semantic_client_db::server_source_index::SourceIndexCollectionScope;
use agent_semantic_client_db::server_source_index::WorkspaceSearchGenerationPublicationRequestV1;
use agent_semantic_client_db::server_source_index::publish_workspace_search_generation_v1;

#[test]
fn publication_has_one_typed_transaction_entry() {
    let _publication: for<'a> fn(
        WorkspaceSearchGenerationPublicationRequestV1<'a>,
    ) -> Result<PublishedSourceIndexGenerationV1, String> = publish_workspace_search_generation_v1;
}

#[test]
fn publication_request_carries_explicit_collection_scope() {
    fn consume_scope(request: WorkspaceSearchGenerationPublicationRequestV1<'_>) {
        let SourceIndexCollectionScope::CompleteGeneration = request.collection_scope;
    }

    let _typed_scope_consumer: for<'a> fn(WorkspaceSearchGenerationPublicationRequestV1<'a>) =
        consume_scope;
}

#[test]
fn publication_receipt_exposes_one_complete_generation_identity() {
    fn consume_receipt(receipt: PublishedSourceIndexGenerationV1) {
        let PublishedSourceIndexGenerationV1 {
            exact_selector_fixture_publication:
                agent_semantic_search::exact_selector_fixture_publication::ExactSelectorFixturePublicationReceiptV1 { .. },
            generation_directory: _,
            provider_envelope_path: _,
            provider_relation_path: _,
            provider_relation_digest: _,
            exact_selector_fixture_path: _,
            generation_digest: _,
            fixture_digest: _,
            workspace_identity_digest: _,
        } = receipt;
    }

    let _typed_receipt_consumer: fn(PublishedSourceIndexGenerationV1) = consume_receipt;
}
