// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub use super::query_model::{RuntimePerformanceQuery, RuntimePerformanceQueryReceipt};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn query_runtime_performance(
    socket_path: &Path,
    query: &RuntimePerformanceQuery,
) -> Result<RuntimePerformanceQueryReceipt, String> {
    query.validate()?;
    let stream = tokio::net::UnixStream::connect(socket_path)
        .await
        .map_err(|error| format!("failed to connect Runtime Server telemetry query: {error}"))?;
    let (reader, mut writer) = stream.into_split();
    let mut packet = serde_json::to_vec(query)
        .map_err(|error| format!("failed to encode Runtime Server performance query: {error}"))?;
    packet.push(b'\n');
    writer
        .write_all(&packet)
        .await
        .map_err(|error| format!("failed to write Runtime Server performance query: {error}"))?;
    // The newline is the complete request frame. Do not race the resident
    // server's response/close with a redundant SHUT_WR: on macOS that can
    // surface ENOTCONN after the request was accepted but before its receipt
    // is read.
    drop(writer);
    let mut receipt_line = String::new();
    BufReader::new(reader)
        .read_line(&mut receipt_line)
        .await
        .map_err(|error| format!("failed to read Runtime Server performance receipt: {error}"))?;
    let receipt: RuntimePerformanceQueryReceipt = serde_json::from_str(&receipt_line)
        .map_err(|error| format!("failed to decode Runtime Server performance receipt: {error}"))?;
    receipt.validate()?;
    Ok(receipt)
}
