// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::ProjectWorkspaceBinding;
use agent_semantic_content_identity::content_binding::AuthorityStamp;
use agent_semantic_content_identity::content_binding::ContentBinding;
use agent_semantic_content_identity::content_binding::ContentIdentity;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBindingInput;
use agent_semantic_search_projection::QueryPlaybookMaterializationReceipt;
use agent_semantic_search_projection::QueryPlaybookMaterializationRequest;

const PROJECT_WORKSPACE: &str =
    "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root";
const WORKTREE_INSTANCE: &str = "workspace-23cc5ba784c605ae";
const EXECUTION_PUBLICATION_DIGEST: &str =
    "blake3-256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const RUNTIME_BUNDLE_DIGEST: &str =
    "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn runtime_binding() -> RuntimeExecutionBinding {
    let identity = ContentIdentity {
        runtime_artifact_digest: digest('1'),
        workspace_snapshot_digest: digest('2'),
        source_generation_digest: digest('3'),
        source_index_digest: digest('4'),
        schema_digest: digest('5'),
        provider_catalog_digest: digest('6'),
    };
    let binding = ContentBinding::new(
        identity.clone(),
        AuthorityStamp {
            key_id: "runtime-authority".to_owned(),
            canonical_digest: identity.digest(),
            signature: "test-signature".to_owned(),
        },
    )
    .expect("valid content binding");
    RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_workspace: ProjectWorkspaceBinding::new(
            PROJECT_WORKSPACE,
            ".",
            "cross-machine",
            Vec::new(),
        )
        .expect("Project Workspace"),
        worktree_instance_id: WORKTREE_INSTANCE.to_owned(),
        publication_nonce: "publication-1".to_owned(),
        content_binding: binding,
        runtime_artifact_digest: digest('1').into(),
        evaluator_policy_digest: digest('7').into(),
        active_artifact_receipt_digest: digest('8').into(),
        evaluator_abi_digest: digest('9').into(),
    })
    .expect("valid Runtime execution binding")
}

fn manifest_project_workspace(binding: &RuntimeExecutionBinding) -> ProjectWorkspaceBinding {
    binding.project_workspace.clone()
}

fn request(binding: &RuntimeExecutionBinding) -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.query-playbook",
        "protocolVersion": "1",
        "requestId": "query-request-1",
        "projectWorkspaceIdentity": PROJECT_WORKSPACE,
        "worktreeInstanceId": WORKTREE_INSTANCE,
        "runtimeExecutionBinding": binding,
        "runtimeWorkspaceExecutionPublicationDigest": EXECUTION_PUBLICATION_DIGEST,
        "runtimeBundleDigest": RUNTIME_BUNDLE_DIGEST,
        "selectors": [
            "org://docs/publication.org#item/heading/Publication",
            "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry"
        ],
        "projection": "source"
    })
}

fn receipt(binding: &RuntimeExecutionBinding) -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-receipt",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.query-playbook",
        "protocolVersion": "1",
        "requestId": "query-request-1",
        "projectWorkspaceIdentity": PROJECT_WORKSPACE,
        "worktreeInstanceId": WORKTREE_INSTANCE,
        "runtimeExecutionBinding": binding,
        "runtimeWorkspaceExecutionPublicationDigest": EXECUTION_PUBLICATION_DIGEST,
        "runtimeBundleDigest": RUNTIME_BUNDLE_DIGEST,
        "projection": "source",
        "requestedSelectors": [
            "org://docs/publication.org#item/heading/Publication",
            "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry"
        ],
        "materializations": [
            {
                "selector": "org://docs/publication.org#item/heading/Publication",
                "languageId": "org",
                "providerId": "asp-org",
                "ownerPath": "docs/publication.org",
                "projection": "source",
                "gqlRelationships": [{
                    "fromNode": "Publication",
                    "relation": "documents",
                    "toNode": "Registry::publish"
                }],
                "sourceContentDigest": "b".repeat(64),
                "bytes": [42, 32, 80]
            },
            {
                "selector": "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry",
                "languageId": "rust",
                "providerId": "asp-rust",
                "ownerPath": "src/registry.rs",
                "projection": "source",
                "gqlRelationships": [{
                    "fromNode": "Registry::refresh",
                    "relation": "calls",
                    "toNode": "Registry::publish"
                }],
                "sourceContentDigest": "c".repeat(64),
                "bytes": [102, 110]
            }
        ],
        "terminal": {"state": "ready", "terminalCount": 1}
    })
}

#[test]
fn query_playbook_is_independent_of_search_and_accepts_the_smallest_selector_subset() {
    let binding = runtime_binding();
    let mut packet = request(&binding);
    packet["selectors"] = serde_json::json!([
        "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry"
    ]);

    QueryPlaybookMaterializationRequest::admit_for_runtime(
        packet,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect("Query may select one canonical selector without reproducing a Search result");
}

#[test]
fn query_playbook_accepts_polyglot_selectors_learned_by_different_searches() {
    let binding = runtime_binding();
    QueryPlaybookMaterializationRequest::admit_for_runtime(
        request(&binding),
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect("Query may combine independently learned canonical selectors");
}

#[test]
fn query_playbook_rejects_runtime_execution_binding_drift_before_materialization() {
    let binding = runtime_binding();
    let mut packet = request(&binding);
    packet["runtimeExecutionBinding"]["evaluatorPolicyDigest"] = serde_json::json!(digest('a'));

    let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
        packet,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect_err("a changed Runtime binding must fail before provider materialization");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-runtime-binding-mismatch"
    );
}

#[test]
fn query_playbook_rejects_execution_publication_or_outer_bundle_drift() {
    let binding = runtime_binding();
    for field in [
        "runtimeWorkspaceExecutionPublicationDigest",
        "runtimeBundleDigest",
    ] {
        let mut packet = request(&binding);
        packet[field] = serde_json::json!(digest('a'));
        let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
            packet,
            &binding,
            EXECUTION_PUBLICATION_DIGEST,
            RUNTIME_BUNDLE_DIGEST,
            &manifest_project_workspace(&binding),
        )
        .expect_err("Query cannot replay another execution publication or outer bundle");
        assert_eq!(
            error.reason_kind(),
            "query-playbook-execution-publication-mismatch"
        );
    }
}

#[test]
fn query_playbook_rejects_a_foreign_runtime_workspace_against_the_manifest() {
    let manifest_project_workspace = runtime_binding().project_workspace;
    let mut foreign_binding = runtime_binding();
    foreign_binding.project_workspace = ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/foreign.git#workspace/root",
        ".",
        "cross-machine",
        Vec::new(),
    )
    .expect("valid but foreign Project Workspace");

    let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
        request(&foreign_binding),
        &foreign_binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace,
    )
    .expect_err("equal request and Runtime values cannot self-authorize a foreign workspace");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-project-workspace-manifest-mismatch"
    );
}

#[test]
fn query_playbook_rejects_project_or_worktree_context_drift() {
    let binding = runtime_binding();
    for field in ["projectWorkspaceIdentity", "worktreeInstanceId"] {
        let mut packet = request(&binding);
        packet[field] = serde_json::json!("foreign-context");
        let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
            packet,
            &binding,
            EXECUTION_PUBLICATION_DIGEST,
            RUNTIME_BUNDLE_DIGEST,
            &manifest_project_workspace(&binding),
        )
        .expect_err("Query context must equal the admitted Runtime binding");
        assert_eq!(
            error.reason_kind(),
            "query-playbook-runtime-context-mismatch"
        );
    }
}

#[test]
fn query_playbook_rejects_search_only_matches_projection() {
    let binding = runtime_binding();
    let mut packet = request(&binding);
    packet["projection"] = serde_json::json!("matches");

    let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
        packet,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect_err("Query materialization cannot use the Search-only matches projection");
    assert_eq!(error.reason_kind(), "schema-invalid");
}

#[test]
fn query_playbook_receipt_accepts_one_complete_runtime_bound_terminal() {
    let binding = runtime_binding();
    let admitted_request = QueryPlaybookMaterializationRequest::admit_for_runtime(
        request(&binding),
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect("valid request");

    QueryPlaybookMaterializationReceipt::admit_for_runtime(
        receipt(&binding),
        &admitted_request,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect("one complete Ready terminal must be admitted");
}

#[test]
fn query_playbook_receipt_rejects_parallel_gql_relationships_for_one_source_block() {
    let binding = runtime_binding();
    let admitted_request = QueryPlaybookMaterializationRequest::admit_for_runtime(
        request(&binding),
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect("valid request");
    let mut packet = receipt(&binding);
    packet["materializations"][0]["gqlRelationships"] = serde_json::json!([
        {
            "fromNode": "Publication",
            "relation": "documents",
            "toNode": "Registry::publish"
        },
        {
            "fromNode": "Query",
            "relation": "materializes",
            "toNode": "Publication"
        }
    ]);
    let error = QueryPlaybookMaterializationReceipt::admit_for_runtime(
        packet,
        &admitted_request,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect_err("one source block has exactly one immediately preceding GQL relation");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-gql-relationship-invalid"
    );
}

#[test]
fn query_playbook_receipt_rejects_partial_ready_and_partial_failure() {
    let binding = runtime_binding();
    let admitted_request = QueryPlaybookMaterializationRequest::admit_for_runtime(
        request(&binding),
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect("valid request");

    let mut partial_ready = receipt(&binding);
    partial_ready["materializations"] =
        serde_json::json!([partial_ready["materializations"][0].clone()]);
    let ready_error = QueryPlaybookMaterializationReceipt::admit_for_runtime(
        partial_ready,
        &admitted_request,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect_err("Ready cannot omit a requested selector");
    assert_eq!(
        ready_error.reason_kind(),
        "query-playbook-materialization-set-mismatch"
    );

    let mut partial_failure = receipt(&binding);
    partial_failure["terminal"] = serde_json::json!({
        "state": "failed",
        "terminalCount": 1,
        "reasonKind": "selector-stale"
    });
    let failure_error = QueryPlaybookMaterializationReceipt::admit_for_runtime(
        partial_failure,
        &admitted_request,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &manifest_project_workspace(&binding),
    )
    .expect_err("Failed cannot expose partial materialization");
    assert_eq!(
        failure_error.reason_kind(),
        "query-playbook-failure-exposed-partial-materialization"
    );
}
