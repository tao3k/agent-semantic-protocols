// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{AtomicSnapshotPointerWriter, POINTER_LEN};

#[tokio::test]
async fn existing_invalid_pointer_is_replaced_without_truncation() {
    let root = tempfile::tempdir().expect("atomic pointer fixture");
    let path = root.path().join("pointer.memory");
    tokio::fs::write(&path, vec![7_u8; 31])
        .await
        .expect("write invalid existing pointer");

    let old_inode = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&path)
        .await
        .expect("retain old pointer inode");
    let writer = AtomicSnapshotPointerWriter::open(path.clone(), "test pointer")
        .await
        .expect("invalid existing pointer should be replaced by inode");
    assert_eq!(
        old_inode
            .metadata()
            .await
            .expect("inspect retained old pointer inode")
            .len(),
        31
    );
    assert_eq!(
        tokio::fs::metadata(writer.path())
            .await
            .expect("inspect initialized pointer")
            .len(),
        POINTER_LEN as u64
    );
}
