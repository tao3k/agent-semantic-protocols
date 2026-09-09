// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! State Core admission and compact rendering helpers for the DB facade.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::state_core::STATE_MANIFEST_FILE;

use crate::types::{
    ClientDbArtifactGraphCompactRender, ClientDbArtifactRepairChainFrame, ClientDbArtifactRoot,
    ClientDbProofReceipt,
};

use super::contract::ClientDbBackend;

pub(super) fn prepare_client_dir_for_write(client_dir: &Path) -> Result<(), String> {
    require_state_core_materialization(client_dir)?;
    fs::create_dir_all(client_dir).map_err(|error| {
        format!(
            "failed to create DB Engine client dir `{}`: {error}",
            client_dir.display()
        )
    })
}

pub(super) fn require_state_core_materialization(client_dir: &Path) -> Result<(), String> {
    if let Some(workspace_digest) = canonical_workspace_digest(client_dir) {
        let binding_path = client_dir
            .join("observations")
            .join(agent_semantic_artifacts::WORKSPACE_BINDING_FILE);
        let bytes = fs::read(&binding_path).map_err(|error| {
            format!(
                "state-home-binding-required: DB Engine write requires {}: {error}",
                binding_path.display()
            )
        })?;
        let binding = serde_json::from_slice::<agent_semantic_artifacts::ProjectBinding>(&bytes)
            .map_err(|error| {
                format!(
                    "state-home-binding-invalid: decode {}: {error}",
                    binding_path.display()
                )
            })?;
        binding.validate()?;
        let expected = binding
            .workspace
            .digest
            .as_str()
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "workspace binding digest is not canonical".to_string())?;
        if expected != workspace_digest {
            return Err(format!(
                "state-home-binding-mismatch: directoryDigest={workspace_digest} bindingDigest={expected}"
            ));
        }
        return Ok(());
    }
    let Some((project_dir, workspace_dir)) = state_core_identity_dirs(client_dir) else {
        return Ok(());
    };
    let required = [
        project_dir.join("project.json"),
        workspace_dir.join("workspace.json"),
        client_dir.join(STATE_MANIFEST_FILE),
    ];
    if required.iter().all(|path| path.is_file()) {
        return Ok(());
    }
    let missing = required
        .iter()
        .filter(|path| !path.is_file())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "state-core-materialization-required: DB Engine write requires committed identity metadata before mutating `{}`; missing: {missing}",
        client_dir.display()
    ))
}

fn canonical_workspace_digest(client_dir: &Path) -> Option<&str> {
    let digest = client_dir.file_name()?.to_str()?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    (client_dir.parent()?.file_name()?.to_str()? == "workspaces").then_some(digest)
}

fn state_core_identity_dirs(client_dir: &Path) -> Option<(PathBuf, PathBuf)> {
    if client_dir.file_name()?.to_str()? != "client" {
        return None;
    }
    let live_dir = client_dir.parent()?;
    if live_dir.file_name()?.to_str()? != "live" {
        return None;
    }
    let workspace_dir = live_dir.parent()?;
    let workspaces_dir = workspace_dir.parent()?;
    if workspaces_dir.file_name()?.to_str()? != "workspaces" {
        return None;
    }
    let project_dir = workspaces_dir.parent()?;
    let by_id_dir = project_dir.parent()?;
    if by_id_dir.file_name()?.to_str()? != "by-id" {
        return None;
    }
    let projects_dir = by_id_dir.parent()?;
    if projects_dir.file_name()?.to_str()? != "projects" {
        return None;
    }
    Some((project_dir.to_path_buf(), workspace_dir.to_path_buf()))
}

pub(super) fn render_artifact_graph_compact_lines(
    frames: &[ClientDbArtifactRepairChainFrame],
    receipts: &[ClientDbProofReceipt],
) -> ClientDbArtifactGraphCompactRender {
    let mut lines = vec![format!(
        "|artifactGraph frameCount={} proofReceiptCount={} disclosure=compact",
        frames.len(),
        receipts.len()
    )];
    for frame in frames {
        lines.push(format!(
            "|repairFrame kind={} root={} content={} parents={}",
            compact_atom(&frame.frame_kind),
            compact_root_ref(&frame.root),
            compact_hash(&frame.content_hash),
            frame.parents.len()
        ));
        for edge in &frame.parents {
            lines.push(format!(
                "|artifactEdge role={} parent={} child={} edge={}",
                compact_atom(&edge.role),
                compact_root_ref(&edge.parent),
                compact_root_ref(&edge.child),
                compact_hash(&edge.edge_hash)
            ));
        }
    }
    for receipt in receipts {
        lines.push(format!(
            "|proofReceipt id={} ok={} trust={} root={} summary=\"{}\"",
            compact_atom(&receipt.receipt_id),
            receipt.okay,
            compact_atom(&receipt.trust_level),
            compact_root_ref(&receipt.root),
            compact_text(&receipt.summary_for_agent)
        ));
    }
    ClientDbArtifactGraphCompactRender {
        frame_count: u32::try_from(frames.len()).unwrap_or(u32::MAX),
        proof_receipt_count: u32::try_from(receipts.len()).unwrap_or(u32::MAX),
        lines,
    }
}

fn compact_root_ref(root: &ClientDbArtifactRoot) -> String {
    format!(
        "{}@{}",
        compact_atom(&root.root_kind),
        compact_hash(&root.root_hash)
    )
}

fn compact_hash(hash: &crate::ClientDbArtifactHash) -> String {
    let prefix: String = hash.value.chars().take(16).collect();
    format!("{}:{}", compact_atom(&hash.algorithm), prefix)
}

fn compact_atom(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | ':' | '/' | '.' | '@')
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn compact_text(value: &str) -> String {
    let mut compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    compact = compact.replace('"', "'");
    if compact.len() > 160 {
        compact.truncate(157);
        compact.push_str("...");
    }
    compact
}

pub(super) fn active_client_db_backend() -> ClientDbBackend {
    ClientDbBackend::Turso
}
