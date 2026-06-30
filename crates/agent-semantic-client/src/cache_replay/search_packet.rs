//! Search packet artifact replay through the local graph renderer.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_runtime::{
    GraphRenderReceiptRequest, run_graph_render_packet, run_graph_render_packet_bytes,
    run_graph_render_packet_bytes_with_receipt,
};
use agent_semantic_search::{
    output_with_delegation_hint_lines, search_output_artifact_replay_safe,
};
use bytes::Bytes;

use super::limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;

pub(crate) struct SearchFrontierReceiptRequest {
    pub(crate) out_path: PathBuf,
    pub(crate) receipt_id: String,
    pub(crate) task_fingerprint: String,
    pub(crate) command_fingerprint: String,
}

pub(crate) fn render_search_packet_bytes(packet_bytes: Bytes) -> Option<Bytes> {
    if packet_bytes.is_empty() || packet_bytes.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let output =
        run_graph_render_packet_bytes(packet_bytes.clone(), MAX_CACHE_REPLAY_ARTIFACT_BYTES)?;
    let output = output_with_delegation_hint_lines(output, &packet_bytes);
    if !search_output_artifact_replay_safe(&output) {
        return None;
    }
    Some(output)
}

pub(crate) fn render_search_packet_bytes_with_receipt(
    packet_bytes: Bytes,
    receipt: &SearchFrontierReceiptRequest,
) -> Result<Option<Bytes>, String> {
    if packet_bytes.is_empty() || packet_bytes.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return Ok(None);
    }
    let output = run_graph_render_packet_bytes_with_receipt(
        packet_bytes.clone(),
        MAX_CACHE_REPLAY_ARTIFACT_BYTES,
        &GraphRenderReceiptRequest {
            out_path: receipt.out_path.clone(),
            receipt_id: receipt.receipt_id.clone(),
            task_fingerprint: receipt.task_fingerprint.clone(),
            command_fingerprint: receipt.command_fingerprint.clone(),
        },
    )?;
    let Some(output) = output else {
        return Ok(None);
    };
    if !search_output_artifact_replay_safe(&output) {
        return Ok(None);
    }
    Ok(Some(output_with_delegation_hint_lines(
        output,
        &packet_bytes,
    )))
}

pub(crate) fn render_search_packet_artifact_stdout(artifact_path: &Path) -> Option<Bytes> {
    let metadata = fs::metadata(artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    run_graph_render_packet(artifact_path, MAX_CACHE_REPLAY_ARTIFACT_BYTES)
}
