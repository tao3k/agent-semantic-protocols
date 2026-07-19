use super::ContentAddressedStore;

#[test]
fn immutable_payload_round_trips_by_digest() {
    let root = std::env::temp_dir().join(format!(
        "asp-content-store-{}-{}",
        std::process::id(),
        "round-trip"
    ));
    let store = ContentAddressedStore::new(&root);
    let digest = "a".repeat(64);

    store.write(&digest, b"artifact").expect("write artifact");
    assert_eq!(
        store.read(&digest).expect("read artifact"),
        Some(b"artifact".to_vec())
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalid_digest_cannot_escape_store_root() {
    let store = ContentAddressedStore::new(std::env::temp_dir());
    let error = store
        .write("../escape", b"artifact")
        .expect_err("invalid digest rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}
