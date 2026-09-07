// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_semantic_content_identity::Blake3DigestV1;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use agent_semantic_topology::{
    ProjectTopologyLibrary, ProjectTopologyManifest, RuntimeProjectTopologyAttachment,
};

fn attachment_packet() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/runtime-project-topology-attachment/valid-current-generation.v1.json"
    ))
    .expect("valid Runtime Project Topology attachment fixture")
}

fn library_packet() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/project-topology-library/valid-polyglot.v1.json"
    ))
    .expect("valid Project Topology fixture")
}

fn manifest() -> ProjectTopologyManifest {
    ProjectTopologyManifest::parse_org(include_str!(
        "../../../../org/templates/project.topology-program.v1.org"
    ))
    .expect("Project Topology manifest")
}

fn admitted_library() -> ProjectTopologyLibrary {
    let packet = library_packet();
    let admitted = BTreeMap::from([(
        "topology-rebuild-1".to_owned(),
        packet["fromScratchRebuildReceipt"].clone(),
    )]);
    ProjectTopologyLibrary::admit_with_receipts(packet, manifest().project_workspace(), &admitted)
        .expect("complete topology library")
}

fn admit_attachment(
    packet: &serde_json::Value,
    admitted_receipts: &BTreeMap<String, serde_json::Value>,
) -> Result<
    RuntimeProjectTopologyAttachment,
    agent_semantic_topology::RuntimeProjectTopologyAttachmentError,
> {
    let mut admitted_packet = packet.clone();
    let mut runtime_binding: RuntimeExecutionBinding =
        serde_json::from_value(admitted_packet["runtimeExecutionBinding"].clone())
            .expect("Runtime execution binding");
    runtime_binding
        .content_binding
        .authority_stamp
        .canonical_digest = runtime_binding.content_binding.identity.digest();
    admitted_packet["runtimeExecutionBinding"] =
        serde_json::to_value(&runtime_binding).expect("Runtime execution binding serializes");
    RuntimeProjectTopologyAttachment::admit(
        admitted_packet,
        Blake3DigestV1::from(
            packet["runtimeGenerationDigest"]
                .as_str()
                .expect("Runtime generation digest"),
        ),
        runtime_binding,
        Arc::new(admitted_library()),
        admitted_receipts,
    )
}

fn independently_admitted_receipt(
    packet: &serde_json::Value,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([(
        packet["inferenceReceipt"]["receiptDigest"]
            .as_str()
            .expect("receipt digest")
            .to_owned(),
        packet["inferenceReceipt"].clone(),
    )])
}

#[test]
fn runtime_attachment_admits_the_exact_complete_identity_product() {
    let packet = attachment_packet();
    let attachment = admit_attachment(&packet, &independently_admitted_receipt(&packet))
        .expect("exact attachment must be admitted");

    assert_eq!(
        attachment.runtime_generation_digest(),
        packet["runtimeGenerationDigest"].as_str().unwrap()
    );
    assert_eq!(
        attachment.library().library_digest(),
        packet["topologyLibraryBinding"]["libraryDigest"]
            .as_str()
            .unwrap()
    );
}

#[test]
fn runtime_attachment_rejects_an_embedded_but_unadmitted_inference_receipt() {
    let packet = attachment_packet();
    let error = admit_attachment(&packet, &BTreeMap::new())
        .expect_err("an embedded receipt cannot admit itself");
    assert_eq!(
        error.reason_kind(),
        "runtime-topology-inference-receipt-unadmitted"
    );
}

#[test]
fn runtime_attachment_rejects_source_provider_and_workspace_drift() {
    for mutation in ["source", "provider", "workspace"] {
        let mut packet = attachment_packet();
        match mutation {
            "source" => {
                packet["runtimeExecutionBinding"]["contentBinding"]["sourceGenerationDigest"] = serde_json::json!(
                    "blake3-256:9999999999999999999999999999999999999999999999999999999999999999"
                );
            }
            "provider" => {
                packet["runtimeExecutionBinding"]["contentBinding"]["providerCatalogDigest"] = serde_json::json!(
                    "blake3-256:9999999999999999999999999999999999999999999999999999999999999999"
                );
            }
            "workspace" => {
                packet["runtimeExecutionBinding"]["projectWorkspace"]["projectWorkspaceIdentity"] = serde_json::json!(
                    "git+https://github.com/tao3k/another-project.git#workspace/root"
                );
            }
            _ => unreachable!(),
        }
        let error = admit_attachment(&packet, &independently_admitted_receipt(&packet))
            .expect_err("binding drift must be rejected");
        assert_eq!(
            error.reason_kind(),
            "runtime-project-topology-binding-mismatch"
        );
    }
}

#[test]
fn runtime_attachment_rejects_generation_or_artifact_replay() {
    for field in ["runtimeGenerationDigest", "runtimeArtifactDigest"] {
        let mut packet = attachment_packet();
        packet["inferenceReceipt"][field] = serde_json::json!(
            "blake3-256:9999999999999999999999999999999999999999999999999999999999999999"
        );
        let admitted = independently_admitted_receipt(&packet);
        let error = admit_attachment(&packet, &admitted)
            .expect_err("receipt replay across Runtime identity must fail");
        assert_eq!(
            error.reason_kind(),
            "runtime-project-topology-binding-mismatch"
        );
    }
}
