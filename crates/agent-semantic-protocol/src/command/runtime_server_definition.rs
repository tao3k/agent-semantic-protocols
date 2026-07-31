use std::path::Path;

use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "resident supervisor path has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        format!(
            "failed to create resident supervisor directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = path.with_extension(format!("tmp-{}", entropy_digest().await?));
    tokio::fs::write(&temporary, bytes).await.map_err(|error| {
        format!(
            "failed to write resident supervisor temporary file {}: {error}",
            temporary.display()
        )
    })?;
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        format!(
            "failed to publish resident supervisor definition {}: {error}",
            path.display()
        )
    })
}

pub(crate) async fn atomic_write_if_changed(
    path: &Path,
    bytes: &[u8],
) -> Result<bool, String> {
    match tokio::fs::read(path).await {
        Ok(current) if current == bytes => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect resident supervisor definition {}: {error}",
                path.display()
            ));
        }
    }
    atomic_write(path, bytes).await?;
    Ok(true)
}

async fn entropy_digest() -> Result<String, String> {
    let mut source = tokio::fs::File::open("/dev/urandom")
        .await
        .map_err(|error| format!("failed to open OS entropy source: {error}"))?;
    let mut entropy = [0_u8; 16];
    source
        .read_exact(&mut entropy)
        .await
        .map_err(|error| format!("failed to read OS entropy: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(entropy)))
}
