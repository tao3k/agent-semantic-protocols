#[cfg(unix)]
pub(super) fn client_dir_snapshot(
    root: &std::path::Path,
) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut snapshot = std::fs::read_dir(root)
        .expect("read client directory snapshot")
        .map(|entry| {
            let entry = entry.expect("read client directory entry");
            let metadata = entry.metadata().expect("read client entry metadata");
            (
                entry.file_name().to_string_lossy().into_owned(),
                metadata.len(),
                metadata.modified().expect("read client entry mtime"),
            )
        })
        .collect::<Vec<_>>();
    snapshot.sort_by(|left, right| left.0.cmp(&right.0));
    snapshot
}
