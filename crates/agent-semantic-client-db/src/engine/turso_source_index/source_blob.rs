use crate::source_index::ClientDbSourceIndexImport;

pub(super) async fn write_source_index_blobs(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
) -> Result<(), String> {
    let file_hashes = import.file_hashes.iter().try_fold(
        std::collections::BTreeMap::new(),
        |mut index, file_hash| {
            if index.insert(file_hash.path.as_str(), file_hash).is_some() {
                return Err(format!(
                    "source-index file hashes repeat owner path: ownerPath={}",
                    file_hash.path
                ));
            }
            Ok(index)
        },
    )?;
    for (owner_path, source_bytes) in import.source_blobs.iter() {
        let file_hash = file_hashes.get(owner_path).ok_or_else(|| {
            format!("source-index blob is missing admitted file hash: ownerPath={owner_path}")
        })?;
        let size_bytes = i64::try_from(source_bytes.len()).unwrap_or(i64::MAX);
        if file_hash.byte_len != source_bytes.len() as u64 {
            return Err(format!(
                "source-index blob size differs from admitted file hash: ownerPath={owner_path} expected={} actual={}",
                file_hash.byte_len,
                source_bytes.len()
            ));
        }
        connection
            .execute(
                "INSERT INTO asp_source_index_blob_v1 (content_digest, size_bytes, source_bytes) VALUES (?1, ?2, ?3) ON CONFLICT (content_digest) DO NOTHING",
                (file_hash.sha256.as_str(), size_bytes, source_bytes),
            )
            .await
            .map_err(|error| {
                format!(
                    "failed to persist content-addressed source-index blob: ownerPath={owner_path} digest={} error={error}",
                    file_hash.sha256
                )
            })?;
    }
    Ok(())
}
