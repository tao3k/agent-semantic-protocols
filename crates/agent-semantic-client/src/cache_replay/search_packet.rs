//! Search packet artifact replay through the local graph renderer.

use std::fs;
use std::path::{Path, PathBuf};

use bytes::Bytes;

use super::graph_render::{
    GraphRenderReceiptRequest, run_graph_render_packet, run_graph_render_packet_bytes,
    run_graph_render_packet_bytes_with_receipt,
};
use super::limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;

pub(crate) struct SearchFrontierReceiptRequest {
    pub(crate) out_path: PathBuf,
    pub(crate) receipt_id: String,
    pub(crate) task_fingerprint: String,
    pub(crate) command_fingerprint: String,
}

pub(crate) fn search_output_artifact_replay_safe(stdout: &[u8]) -> bool {
    let Ok(stdout) = std::str::from_utf8(stdout) else {
        return false;
    };
    let Some(header) = stdout.lines().next() else {
        return false;
    };
    let has_frontier_header = header.contains("[graph-frontier]") || header.starts_with("[search-");
    if header.starts_with("[search-prime]") && !stdout.contains("|decision purpose=decision-primer")
    {
        return false;
    }
    let has_alias_graph = stdout.contains("aliases=");
    has_frontier_header
        && has_alias_graph
        && stdout.contains("legend: ID=kind:role(value)!next;")
        && stdout.contains("frontier ID.next")
        && !stdout.contains('\0')
}

pub(crate) fn render_search_packet_bytes(packet_bytes: Bytes) -> Option<Bytes> {
    if packet_bytes.is_empty() || packet_bytes.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let output = run_graph_render_packet_bytes(packet_bytes, MAX_CACHE_REPLAY_ARTIFACT_BYTES)?;
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
        packet_bytes,
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
    Ok(Some(output))
}

pub(crate) fn render_search_packet_artifact_stdout(artifact_path: &Path) -> Option<Bytes> {
    let metadata = fs::metadata(artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    run_graph_render_packet(artifact_path, MAX_CACHE_REPLAY_ARTIFACT_BYTES)
}
