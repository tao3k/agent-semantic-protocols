// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn generic_client_runtime_does_not_embed_or_directly_depend_on_a_language_provider() {
    let manifest: toml::Value =
        toml::from_str(include_str!("../../Cargo.toml")).expect("agent-semantic-client manifest");
    for dependency_class in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(dependencies) = manifest
            .get(dependency_class)
            .and_then(toml::Value::as_table)
        else {
            continue;
        };
        assert!(
            !dependencies.contains_key("asp-rust"),
            "the generic client must reach Rust through provider transport, not an {dependency_class} asp-rust dependency"
        );
    }

    let service = include_str!("../../src/server/runtime_server_search_service.rs");
    assert!(
        !service.contains("asp_rust::"),
        "the Runtime search service must not embed the Rust provider implementation"
    );

    let workspace_manifest: toml::Value =
        toml::from_str(include_str!("../../../../Cargo.toml")).expect("workspace manifest");
    let workspace_policy =
        workspace_manifest["workspace"]["dependencies"]["asp-rust-project-harness-policy"]
            .as_table()
            .expect("workspace policy dependency");
    assert_eq!(
        workspace_policy
            .get("default-features")
            .and_then(toml::Value::as_bool),
        Some(false),
        "generic workspace members must use the manifest-only policy path instead of compiling asp-rust"
    );

    let hook_testkit_manifest: toml::Value = toml::from_str(include_str!(
        "../../../agent-semantic-hook-testkit/Cargo.toml"
    ))
    .expect("Hook TestKit manifest");
    let hook_testkit_policy =
        hook_testkit_manifest["dependencies"]["asp-rust-project-harness-policy"]
            .as_table()
            .expect("Hook TestKit policy dependency");
    assert_eq!(
        hook_testkit_policy
            .get("default-features")
            .and_then(toml::Value::as_bool),
        Some(false),
        "the client dev graph must not regain asp-rust through Hook TestKit"
    );
}
