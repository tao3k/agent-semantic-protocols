//! Filesystem-backed content-addressed artifact storage.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Store immutable artifact payloads by their lowercase hexadecimal digest.
#[derive(Debug, Clone)]
pub struct ContentAddressedStore {
    root: PathBuf,
}

impl ContentAddressedStore {
    /// Create a store rooted at the supplied directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Return the configured store root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Load an artifact payload when the digest is present.
    pub fn read(&self, digest: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.artifact_path(digest)?;
        match fs::read(path) {
            Ok(payload) => Ok(Some(payload)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Atomically publish an immutable artifact payload.
    pub fn write(&self, digest: &str, payload: &[u8]) -> io::Result<PathBuf> {
        let path = self.artifact_path(digest)?;
        if path.try_exists()? {
            let existing = fs::read(&path)?;
            if existing == payload {
                return Ok(path);
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("content-addressed payload mismatch for digest {digest}"),
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("artifact path has no parent"))?;
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".{}.{}.tmp", digest, std::process::id()));
        fs::write(&temporary, payload)?;
        match fs::rename(&temporary, &path) {
            Ok(()) => Ok(path),
            Err(_error) if path.try_exists()? => {
                let _ = fs::remove_file(temporary);
                Ok(path)
            }
            Err(error) => {
                let _ = fs::remove_file(temporary);
                Err(error)
            }
        }
    }

    fn artifact_path(&self, digest: &str) -> io::Result<PathBuf> {
        validate_digest(digest)?;
        Ok(self.root.join(&digest[..2]).join(&digest[2..]))
    }
}

fn validate_digest(digest: &str) -> io::Result<()> {
    if digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "artifact digest must be 64 lowercase hexadecimal characters",
    ))
}
