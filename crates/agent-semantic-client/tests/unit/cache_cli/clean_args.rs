// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::clean_args::parse_cache_clean_args;

#[test]
fn clean_day_without_value_retains_one_day() {
    let parsed = parse_cache_clean_args(&["--day".to_string()])
        .expect("parse clean arguments")
        .expect("non-help invocation");
    assert_eq!(parsed.day, 1);
}

#[test]
fn clean_day_accepts_a_larger_explicit_retention() {
    let parsed = parse_cache_clean_args(&["--day".to_string(), "14".to_string()])
        .expect("parse clean arguments")
        .expect("non-help invocation");
    assert_eq!(parsed.day, 14);
}

#[test]
fn clean_accepts_one_exact_catalog_selection_axis() {
    let digest = "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let by_workspace = parse_cache_clean_args(&[
        "--day".to_string(),
        "--workspace-digest".to_string(),
        digest.to_string(),
    ])
    .expect("parse workspace selection")
    .expect("non-help invocation");
    assert_eq!(by_workspace.workspace_digest.as_deref(), Some(digest));
    assert!(by_workspace.workspace_root.is_none());
    assert!(by_workspace.object_id.is_none());

    let by_root = parse_cache_clean_args(&[
        "--day".to_string(),
        "--workspace-root".to_string(),
        "/workspace/retired".to_string(),
    ])
    .expect("parse workspace-root selection")
    .expect("non-help invocation");
    assert_eq!(
        by_root.workspace_root.as_deref(),
        Some(std::path::Path::new("/workspace/retired"))
    );

    let by_object = parse_cache_clean_args(&[
        "--day".to_string(),
        "--object-id".to_string(),
        "workspace-cache:retired".to_string(),
    ])
    .expect("parse object selection")
    .expect("non-help invocation");
    assert_eq!(
        by_object.object_id.as_deref(),
        Some("workspace-cache:retired")
    );
}

#[test]
fn clean_rejects_multiple_selection_axes() {
    let error = parse_cache_clean_args(&[
        "--day".to_string(),
        "--workspace-root".to_string(),
        "/workspace/retired".to_string(),
        "--object-id".to_string(),
        "workspace-cache:retired".to_string(),
    ])
    .expect_err("cleanup selection must be singular");
    assert!(error.contains("cannot be used with"), "{error}");
}

#[test]
fn clean_requires_a_positive_day_retention() {
    let missing = parse_cache_clean_args(&[]).expect_err("--day should be required");
    assert!(missing.contains("--day [<DAYS>]"), "{missing}");

    let zero = parse_cache_clean_args(&["--day=0".to_string()])
        .expect_err("zero-day cleanup should be rejected");
    assert!(zero.contains("invalid value '0'"), "{zero}");
    assert!(zero.contains("--day"), "{zero}");
}
