//! Streaming digest encoding for immutable workspace generations.

use serde::Serialize;

struct Blake3Writer(blake3::Hasher);

impl std::io::Write for Blake3Writer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) fn typed_digest<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let mut writer =
        std::io::BufWriter::with_capacity(64 * 1024, Blake3Writer(blake3::Hasher::new()));
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| format!("encode workspace generation digest input: {error}"))?;
    let hasher = writer
        .into_inner()
        .map_err(|error| format!("flush workspace generation digest input: {error}"))?
        .0;
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
}
