// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::ProjectWorkspaceBinding;
use agent_semantic_content_identity::content_binding::{
    AuthorityStamp, ContentBinding, ContentIdentity,
};
use agent_semantic_content_identity::runtime_execution::{
    RuntimeExecutionBinding, RuntimeExecutionBindingInput,
};
use agent_semantic_search_projection::{
    QueryPlaybookMaterializationReceipt, QueryPlaybookMaterializationRequest,
};

const PROJECT_WORKSPACE: &str =
    "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root";
const WORKTREE_INSTANCE: &str = "workspace-23cc5ba784c605ae";
const EXECUTION_PUBLICATION_DIGEST: &str =
    "blake3-256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const RUNTIME_BUNDLE_DIGEST: &str =
    "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const RUST_SELECTOR: &str =
    "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry";
const ORG_SELECTOR: &str = "org://docs/publication.org#item/heading/Publication";

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn runtime_binding() -> RuntimeExecutionBinding {
    let identity = ContentIdentity {
        runtime_artifact_digest: digest('1'),
        workspace_snapshot_digest: digest('2'),
        source_generation_digest: digest('1'),
        source_index_digest: digest('4'),
        schema_digest: digest('5'),
        provider_catalog_digest: digest('2'),
    };
    let content_binding = ContentBinding::new(
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
        content_binding,
        runtime_artifact_digest: digest('1').into(),
        evaluator_policy_digest: digest('7').into(),
        active_artifact_receipt_digest: digest('8').into(),
        evaluator_abi_digest: digest('9').into(),
    })
    .expect("valid Runtime execution binding")
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
        "selectors": [RUST_SELECTOR, ORG_SELECTOR],
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
        "requestedSelectors": [RUST_SELECTOR, ORG_SELECTOR],
        "materializations": [
            {
                "selector": RUST_SELECTOR,
                "languageId": "rust",
                "providerId": "asp-rust",
                "ownerPath": "src/registry.rs",
                "projection": "source",
                "sourceContentDigest": "c".repeat(64),
                "bytes": [102, 110]
            },
            {
                "selector": ORG_SELECTOR,
                "languageId": "org",
                "providerId": "asp-org",
                "ownerPath": "docs/publication.org",
                "projection": "source",
                "sourceContentDigest": "b".repeat(64),
                "bytes": [42, 32, 80]
            }
        ],
        "terminal": {"state": "ready", "terminalCount": 1}
    })
}

fn admit_request(
    binding: &RuntimeExecutionBinding,
) -> Result<
    QueryPlaybookMaterializationRequest,
    agent_semantic_search_projection::QueryPlaybookMaterializationError,
> {
    QueryPlaybookMaterializationRequest::admit_for_runtime(
        request(binding),
        binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
}

#[test]
fn query_request_preserves_polyglot_caller_order_without_search_topology() {
    let binding = runtime_binding();
    let admitted = admit_request(&binding).expect("ordered selector request");
    assert_eq!(
        admitted.as_json()["selectors"],
        serde_json::json!([RUST_SELECTOR, ORG_SELECTOR])
    );
}

#[test]
fn query_request_rejects_duplicate_selector_without_sorting_the_request() {
    let binding = runtime_binding();
    let mut packet = request(&binding);
    packet["selectors"] = serde_json::json!([RUST_SELECTOR, ORG_SELECTOR, RUST_SELECTOR]);
    let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
        packet,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect_err("duplicate selector");
    assert_eq!(error.reason_kind(), "schema-invalid");
}

#[test]
fn query_request_rejects_runtime_binding_drift_and_retired_topology_fields() {
    let binding = runtime_binding();
    let mut drifted = request(&binding);
    drifted["runtimeExecutionBinding"]["evaluatorPolicyDigest"] = serde_json::json!(digest('a'));
    let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
        drifted,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect_err("Runtime drift");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-runtime-binding-mismatch"
    );

    let mut topology = request(&binding);
    topology["topologyLibraryDigest"] = serde_json::json!(digest('3'));
    let error = QueryPlaybookMaterializationRequest::admit_for_runtime(
        topology,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect_err("Query must not depend on Search topology");
    assert_eq!(error.reason_kind(), "schema-invalid");
}

#[test]
fn query_receipt_accepts_only_the_complete_request_order() {
    let binding = runtime_binding();
    let admitted = admit_request(&binding).expect("request");
    QueryPlaybookMaterializationReceipt::admit_for_runtime(
        receipt(&binding),
        &admitted,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect("complete ordered receipt");

    let mut reversed = receipt(&binding);
    reversed["materializations"]
        .as_array_mut()
        .expect("materializations")
        .reverse();
    let error = QueryPlaybookMaterializationReceipt::admit_for_runtime(
        reversed,
        &admitted,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect_err("completion order cannot replace request order");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-materialization-set-mismatch"
    );
}

#[test]
fn query_receipt_rejects_search_relationship_residue() {
    let binding = runtime_binding();
    let admitted = admit_request(&binding).expect("request");
    let mut packet = receipt(&binding);
    packet["materializations"][0]["gqlRelationships"] = serde_json::json!([]);
    let error = QueryPlaybookMaterializationReceipt::admit_for_runtime(
        packet,
        &admitted,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect_err("Query receipt cannot carry Search GQL");
    assert_eq!(error.reason_kind(), "schema-invalid");
}

#[test]
fn query_receipt_rejects_partial_ready_and_partial_failure() {
    let binding = runtime_binding();
    let admitted = admit_request(&binding).expect("request");
    let mut partial_ready = receipt(&binding);
    partial_ready["materializations"]
        .as_array_mut()
        .expect("materializations")
        .pop();
    let error = QueryPlaybookMaterializationReceipt::admit_for_runtime(
        partial_ready,
        &admitted,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect_err("Ready cannot omit a target");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-materialization-set-mismatch"
    );

    let mut partial_failure = receipt(&binding);
    partial_failure["terminal"] = serde_json::json!({
        "state": "failed",
        "terminalCount": 1,
        "reasonKind": "selector-stale"
    });
    let error = QueryPlaybookMaterializationReceipt::admit_for_runtime(
        partial_failure,
        &admitted,
        &binding,
        EXECUTION_PUBLICATION_DIGEST,
        RUNTIME_BUNDLE_DIGEST,
        &binding.project_workspace,
    )
    .expect_err("Failed cannot expose materializations");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-failure-exposed-partial-materialization"
    );
}
