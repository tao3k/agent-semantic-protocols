//! Framed host-local transport for typed workspace DB owner requests.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::protocol::MAX_FRAME_BYTES;

/// Stable reason kind for host-local IPC permission denial.
pub const HOST_LOCAL_IPC_PERMISSION_DENIED_REASON_KIND: &str = "host-local-ipc-permission-denied";

/// Normalize a Runtime Server data-plane connection failure.
pub fn runtime_server_data_connect_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return format!(
            "failed to connect Runtime Server data endpoint reasonKind={HOST_LOCAL_IPC_PERMISSION_DENIED_REASON_KIND} errorKind=permission-denied"
        );
    }
    format!("failed to connect Runtime Server data endpoint: {error}")
}

/// Test whether a normalized transport failure is a host permission denial.
pub fn is_host_local_ipc_permission_denied(error: &str) -> bool {
    let expected = format!("reasonKind={HOST_LOCAL_IPC_PERMISSION_DENIED_REASON_KIND}");
    error
        .split_ascii_whitespace()
        .any(|field| field == expected)
}

pub(crate) async fn read_frame<T: for<'de> Deserialize<'de>>(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<T, String> {
    read_optional_frame(stream)
        .await?
        .ok_or_else(|| "workspace owner endpoint closed before a frame was received".to_owned())
}

pub(crate) async fn read_optional_frame<T: for<'de> Deserialize<'de>>(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<Option<T>, String> {
    let first_header = match stream.read_u32().await {
        Ok(header) => header,
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read Runtime Server data frame length: {error}"
            ));
        }
    };
    let mut header = first_header;
    let mut body = Vec::new();
    loop {
        let continuation = header & (1 << 31) != 0;
        let length = (header & !(1 << 31)) as usize;
        if length > MAX_FRAME_BYTES {
            return Err("Runtime Server data frame exceeds chunk size limit".to_owned());
        }
        let next_len = body
            .len()
            .checked_add(length)
            .ok_or_else(|| "Runtime Server data message length overflow".to_owned())?;
        u32::try_from(next_len)
            .map_err(|_| "Runtime Server data message length exceeds u32".to_owned())?;
        body.resize(next_len, 0);
        stream
            .read_exact(&mut body[next_len - length..])
            .await
            .map_err(|error| format!("failed to read Runtime Server data frame: {error}"))?;
        if !continuation {
            break;
        }
        header = stream.read_u32().await.map_err(|error| {
            format!("failed to read continued Runtime Server data frame length: {error}")
        })?;
    }
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| format!("failed to decode Runtime Server data message: {error}"))
}

pub(crate) async fn write_frame<T: Serialize>(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &T,
) -> Result<(), String> {
    let body = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode Runtime Server data message: {error}"))?;
    u32::try_from(body.len())
        .map_err(|_| "Runtime Server data message length exceeds u32".to_owned())?;
    if body.is_empty() {
        stream.write_u32(0).await.map_err(|error| {
            format!("failed to write Runtime Server data frame length: {error}")
        })?;
        stream
            .flush()
            .await
            .map_err(|error| format!("failed to flush Runtime Server data frame: {error}"))?;
        return Ok(());
    }
    let chunk_count = body.len().div_ceil(MAX_FRAME_BYTES);
    for (chunk_index, chunk) in body.chunks(MAX_FRAME_BYTES).enumerate() {
        let mut header =
            u32::try_from(chunk.len()).map_err(|_| "data frame length overflow".to_owned())?;
        if chunk_index + 1 < chunk_count {
            header |= 1 << 31;
        }
        stream.write_u32(header).await.map_err(|error| {
            format!("failed to write Runtime Server data frame length: {error}")
        })?;
        stream
            .write_all(chunk)
            .await
            .map_err(|error| format!("failed to write Runtime Server data frame: {error}"))?;
    }
    stream
        .flush()
        .await
        .map_err(|error| format!("failed to flush Runtime Server data frame: {error}"))?;
    Ok(())
}
