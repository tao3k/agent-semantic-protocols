use super::locator_artifact::collect_locator_paths;
use std::collections::BTreeSet;

#[test]
fn collect_locator_paths_accepts_path_equals_tokens() {
    let mut paths = BTreeSet::new();

    collect_locator_paths("|read path=src/lib.rs:4:8 reason=frontier", &mut paths);

    assert_eq!(paths.into_iter().collect::<Vec<_>>(), vec!["src/lib.rs"]);
}
