use super::read;

#[tokio::test]
async fn owner_content_identity_tracks_live_change_missing_and_normalized_path() {
    let temporary = tempfile::tempdir().expect("create owner identity fixture");
    let owner_path = "src/owner.rs";
    let absolute = temporary.path().join(owner_path);
    tokio::fs::create_dir_all(absolute.parent().expect("owner parent"))
        .await
        .expect("create owner parent");
    tokio::fs::write(&absolute, b"fn before() {}\n")
        .await
        .expect("write initial owner");

    let before = read(temporary.path(), owner_path)
        .await
        .expect("read initial owner identity")
        .expect("initial owner exists");
    tokio::fs::write(&absolute, b"fn after() {}\n")
        .await
        .expect("write changed owner");
    let after = read(temporary.path(), owner_path)
        .await
        .expect("read changed owner identity")
        .expect("changed owner exists");
    assert_ne!(before.digest, after.digest);
    assert_ne!(before.bytes, after.bytes);

    tokio::fs::remove_file(&absolute)
        .await
        .expect("remove owner");
    assert!(
        read(temporary.path(), owner_path)
            .await
            .expect("read missing owner identity")
            .is_none()
    );
    assert!(read(temporary.path(), "../outside.rs").await.is_err());
}
