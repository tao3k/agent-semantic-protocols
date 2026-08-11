use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::UnixStream;

use super::{
    RuntimeServerControlReceipt, RuntimeServerControlRequest, RuntimeServerRequestReadError,
};

const MAX_FRAME_BYTES: usize = 1024 * 1024;

fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode Runtime Server frame: {error}"))?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("Runtime Server frame exceeds size limit".to_owned());
    }
    Ok(bytes)
}

pub(crate) async fn read_runtime_server_requests(
    stream: &mut UnixStream,
) -> Result<Vec<RuntimeServerControlRequest>, RuntimeServerRequestReadError> {
    let length = match stream.read_u32().await {
        Ok(length) => length as usize,
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(RuntimeServerRequestReadError::Closed);
        }
        Err(error) => {
            return Err(RuntimeServerRequestReadError::Invalid(format!(
                "failed to read runtime server frame length: {error}"
            )));
        }
    };
    if length > MAX_FRAME_BYTES {
        return Err(RuntimeServerRequestReadError::Invalid(
            "runtime server frame exceeds size limit".to_owned(),
        ));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await.map_err(|error| {
        RuntimeServerRequestReadError::Invalid(format!(
            "failed to read runtime server frame: {error}"
        ))
    })?;
    let requests: Vec<RuntimeServerControlRequest> =
        serde_json::from_slice(&bytes).map_err(|error| {
            RuntimeServerRequestReadError::Invalid(format!(
                "failed to decode runtime server frame: {error}"
            ))
        })?;
    if requests.is_empty() {
        return Err(RuntimeServerRequestReadError::Invalid(
            "runtime server request batch must not be empty".to_owned(),
        ));
    }
    Ok(requests)
}

pub(crate) async fn write_runtime_server_receipts(
    stream: &mut UnixStream,
    receipts: &[RuntimeServerControlReceipt],
) -> Result<(), String> {
    write_frame(stream, &receipts).await
}

pub(super) async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(
    stream: &mut W,
    value: &T,
) -> Result<(), String> {
    let bytes = encode_frame(value)?;
    stream
        .write_u32(
            bytes
                .len()
                .try_into()
                .map_err(|_| "frame length overflow")?,
        )
        .await
        .map_err(|error| format!("failed to write Runtime Server frame length: {error}"))?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|error| format!("failed to write Runtime Server frame: {error}"))
}

pub(super) async fn read_frame<R: AsyncRead + Unpin, T: for<'de> Deserialize<'de>>(
    stream: &mut R,
) -> Result<T, String> {
    let length = stream
        .read_u32()
        .await
        .map_err(|error| format!("failed to read Runtime Server frame length: {error}"))?
        as usize;
    if length > MAX_FRAME_BYTES {
        return Err("Runtime Server frame exceeds size limit".to_owned());
    }
    let mut bytes = vec![0; length];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|error| format!("failed to read Runtime Server frame: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode Runtime Server frame: {error}"))
}
