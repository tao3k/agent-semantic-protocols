// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::provider_execution_command_digest;

#[test]
fn provider_execution_digest_binds_executable_argv_and_artifact() {
    let executable = std::env::current_exe().expect("current test executable");
    let executable = executable.display().to_string();
    let base = crate::provider_execution_command_digest(
        std::slice::from_ref(&executable),
        "sha256:artifact-a",
    )
    .expect("base execution digest");
    let repeated =
        provider_execution_command_digest(std::slice::from_ref(&executable), "sha256:artifact-a")
            .expect("repeated execution digest");
    let changed_argv = provider_execution_command_digest(
        &[executable.clone(), "--serve".to_owned()],
        "sha256:artifact-a",
    )
    .expect("argv-bound execution digest");
    let changed_artifact = provider_execution_command_digest(&[executable], "sha256:artifact-b")
        .expect("artifact-bound execution digest");

    assert_eq!(base, repeated);
    assert!(base.starts_with("sha256:"));
    assert_ne!(base, changed_argv);
    assert_ne!(base, changed_artifact);
}

#[test]
fn provider_execution_digest_rejects_an_empty_command() {
    let error = provider_execution_command_digest(&[], "sha256:artifact")
        .expect_err("empty execution command must be rejected");
    assert!(error.contains("must not be empty"), "{error}");
}
