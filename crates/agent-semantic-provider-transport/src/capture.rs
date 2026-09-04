//! Framed stdout/stderr capture for provider processes.

use std::io;

use bytes::Bytes;
use bytes::BytesMut;
use futures_util::StreamExt;
use sha2::Digest;
use sha2::Sha256;
use tokio::io::AsyncRead;
use tokio::io::AsyncWriteExt;
use tokio_util::codec::BytesCodec;
use tokio_util::codec::FramedRead;
use tokio_util::codec::LengthDelimitedCodec;
use tokio_util::codec::LinesCodec;

use crate::process_contract::OutputFraming;
use crate::process_contract::OutputMode;
use crate::process_contract::ProviderProcessError;

#[derive(Debug, Clone)]
pub(crate) struct LimitedRead {
    pub(crate) bytes: Bytes,
    pub(crate) total_bytes: usize,
    pub(crate) sha256: Option<String>,
    pub(crate) truncated: bool,
}

impl LimitedRead {
    pub(crate) fn empty() -> Self {
        Self {
            bytes: Bytes::new(),
            total_bytes: 0,
            sha256: None,
            truncated: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProviderOutputStream {
    Stdout,
    Stderr,
}

pub(crate) async fn capture_output_stream<R>(
    reader: R,
    limit: Option<usize>,
    stream: ProviderOutputStream,
    output_mode: OutputMode,
    framing: OutputFraming,
) -> Result<LimitedRead, ProviderProcessError>
where
    R: AsyncRead + Unpin,
{
    match framing {
        OutputFraming::Bytes => capture_bytes(reader, limit, stream, output_mode).await,
        OutputFraming::Lines => capture_lines(reader, limit, stream, output_mode).await,
        OutputFraming::LengthDelimited => {
            capture_length_delimited(reader, limit, stream, output_mode).await
        }
    }
}

async fn capture_bytes<R>(
    reader: R,
    limit: Option<usize>,
    stream: ProviderOutputStream,
    output_mode: OutputMode,
) -> Result<LimitedRead, ProviderProcessError>
where
    R: AsyncRead + Unpin,
{
    let mut capture = CaptureAccumulator::new(limit);
    let mut frames = FramedRead::new(reader, BytesCodec::new());
    while let Some(frame) = frames.next().await {
        let frame = frame.map_err(|source| read_error(stream, source))?;
        capture.push(stream, output_mode, frame.as_ref()).await?;
    }
    Ok(capture.finish())
}

async fn capture_lines<R>(
    reader: R,
    limit: Option<usize>,
    stream: ProviderOutputStream,
    output_mode: OutputMode,
) -> Result<LimitedRead, ProviderProcessError>
where
    R: AsyncRead + Unpin,
{
    let mut capture = CaptureAccumulator::new(limit);
    let mut frames = FramedRead::new(reader, LinesCodec::new());
    while let Some(frame) = frames.next().await {
        let line = frame.map_err(|source| {
            read_error(stream, io::Error::new(io::ErrorKind::InvalidData, source))
        })?;
        capture.push(stream, output_mode, line.as_bytes()).await?;
        capture.push(stream, output_mode, b"\n").await?;
    }
    Ok(capture.finish())
}

async fn capture_length_delimited<R>(
    reader: R,
    limit: Option<usize>,
    stream: ProviderOutputStream,
    output_mode: OutputMode,
) -> Result<LimitedRead, ProviderProcessError>
where
    R: AsyncRead + Unpin,
{
    let mut capture = CaptureAccumulator::new(limit);
    let mut frames = FramedRead::new(reader, LengthDelimitedCodec::new());
    while let Some(frame) = frames.next().await {
        let frame = frame.map_err(|source| read_error(stream, source))?;
        capture.push(stream, output_mode, frame.as_ref()).await?;
    }
    Ok(capture.finish())
}

struct CaptureAccumulator {
    bytes: BytesMut,
    total_bytes: usize,
    hasher: Sha256,
    limit: Option<usize>,
}

impl CaptureAccumulator {
    fn new(limit: Option<usize>) -> Self {
        Self {
            bytes: BytesMut::new(),
            total_bytes: 0,
            hasher: Sha256::new(),
            limit,
        }
    }

    async fn push(
        &mut self,
        stream: ProviderOutputStream,
        output_mode: OutputMode,
        chunk: &[u8],
    ) -> Result<(), ProviderProcessError> {
        self.hasher.update(chunk);
        self.total_bytes += chunk.len();
        if output_mode == OutputMode::Tee {
            write_tee(stream, chunk).await?;
        }
        self.retain(chunk);
        Ok(())
    }

    fn retain(&mut self, chunk: &[u8]) {
        let Some(limit) = self.limit else {
            self.bytes.extend_from_slice(chunk);
            return;
        };
        if self.bytes.len() >= limit {
            return;
        }
        let remaining = limit - self.bytes.len();
        let retained = remaining.min(chunk.len());
        self.bytes.extend_from_slice(&chunk[..retained]);
    }

    fn finish(self) -> LimitedRead {
        LimitedRead {
            bytes: self.bytes.freeze(),
            total_bytes: self.total_bytes,
            sha256: Some(format!("{:x}", self.hasher.finalize())),
            truncated: self.limit.is_some_and(|limit| self.total_bytes > limit),
        }
    }
}

fn read_error(stream: ProviderOutputStream, source: io::Error) -> ProviderProcessError {
    match stream {
        ProviderOutputStream::Stdout => ProviderProcessError::StdoutRead { source },
        ProviderOutputStream::Stderr => ProviderProcessError::StderrRead { source },
    }
}

async fn write_tee(stream: ProviderOutputStream, chunk: &[u8]) -> Result<(), ProviderProcessError> {
    match stream {
        ProviderOutputStream::Stdout => {
            let mut stdout = tokio::io::stdout();
            stdout
                .write_all(chunk)
                .await
                .map_err(|source| ProviderProcessError::StdoutTeeWrite { source })?;
            stdout
                .flush()
                .await
                .map_err(|source| ProviderProcessError::StdoutTeeWrite { source })?;
        }
        ProviderOutputStream::Stderr => {
            let mut stderr = tokio::io::stderr();
            stderr
                .write_all(chunk)
                .await
                .map_err(|source| ProviderProcessError::StderrTeeWrite { source })?;
            stderr
                .flush()
                .await
                .map_err(|source| ProviderProcessError::StderrTeeWrite { source })?;
        }
    }
    Ok(())
}
