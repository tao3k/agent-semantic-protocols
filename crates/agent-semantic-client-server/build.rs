// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

fn main() {
    let _policy_receipt =
        asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env();
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let canonical_proto = agent_semantic_schema_manager::SchemaManager::new(&workspace_root)
        .canonical_wire_artifact_path("asp-client-protocol-v1")
        .expect("resolve Schema Manager-owned ASP Client Protocol wire artifact");
    println!("cargo:rerun-if-changed={}", canonical_proto.display());
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("resolve the workspace-owned vendored protoc binary");
    let mut prost_config = tonic_prost_build::Config::new();
    prost_config.protoc_executable(protoc);
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_with_config(
            prost_config,
            &[canonical_proto],
            &[workspace_root.join("schemas")],
        )
        .expect("compile Schema Manager-owned ASP Client Protocol gRPC transport");
}
