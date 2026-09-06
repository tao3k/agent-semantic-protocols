use crate::execute_native_fd_blocks;

fn owners() -> Vec<String> {
    [
        "src/runtime_client.rs",
        "src/host.rs",
        "tests/runtime_client.rs",
        "docs/runtime.md",
        "python/runtime_client.py",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn native_fd_argv_filters_the_complete_resident_inventory() {
    let result = execute_native_fd_blocks(
        &[vec![
            "-t".to_owned(),
            "f".to_owned(),
            "-e".to_owned(),
            "rs".to_owned(),
            "runtime|host".to_owned(),
            "src".to_owned(),
        ]],
        &owners(),
    )
    .unwrap();
    assert_eq!(
        result.candidate_owner_paths,
        vec!["src/host.rs", "src/runtime_client.rs"]
    );
}

#[test]
fn pipe_branches_remain_independent_before_union() {
    let result = execute_native_fd_blocks(
        &[
            vec!["runtime".to_owned(), "src".to_owned()],
            vec!["host".to_owned(), "src".to_owned()],
        ],
        &owners(),
    )
    .unwrap();
    assert_eq!(result.branch_candidate_owner_paths.len(), 2);
    assert_eq!(
        result.candidate_owner_paths,
        vec!["src/host.rs", "src/runtime_client.rs"]
    );
}

#[test]
fn resident_fd_parser_rejects_roots_outside_the_workspace() {
    assert!(
        execute_native_fd_blocks(
            &[vec!["runtime".to_owned(), "../other".to_owned()]],
            &owners(),
        )
        .unwrap_err()
        .contains("inside")
    );
}
