use super::source_snapshot_envelope_file_name;

#[test]
fn envelope_filename_binds_provider_digest() {
    let first = source_snapshot_envelope_file_name("rs-harness", "digest-a");
    let second = source_snapshot_envelope_file_name("rs-harness", "digest-b");

    assert_eq!(first, "rs-harness--digest-a.json");
    assert_eq!(second, "rs-harness--digest-b.json");
    assert_ne!(first, second);
}
