// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;
use std::process::Output;

#[test]
fn root_help_exposes_the_global_server_lifecycle_adapter() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .arg("--help")
        .output()
        .expect("render root help");

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(
        stdout.lines().any(|line| {
            line.trim_start().starts_with("server")
                && line.contains("Manage the Global ASP Runtime Server")
        }),
        "{stdout}"
    );
}

#[test]
fn healthcheck_help_exposes_only_the_global_runtime_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["help", "healthcheck"])
        .output()
        .expect("render healthcheck help");

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(
        stdout.contains("Usage: asp healthcheck [OPTIONS]"),
        "{stdout}"
    );
    assert!(!stdout.contains("PROJECT_ROOT"), "{stdout}");
}

#[test]
fn healthcheck_rejects_project_scoped_legacy_invocation() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["healthcheck", "."])
        .output()
        .expect("run project-scoped legacy healthcheck");

    assert!(!output.status.success(), "stdout: {}", stdout(&output));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("asp healthcheck is Global and accepts no PROJECT_ROOT"),
        "{stderr}"
    );
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
