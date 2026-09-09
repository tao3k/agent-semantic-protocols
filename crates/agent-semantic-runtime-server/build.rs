// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;

fn main() {
    let _policy_report =
        asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env();
    println!("cargo:rerun-if-changed=proto/asp-provider-stream.proto");
    println!("cargo:rerun-if-changed=proto/asp-python-graphs.proto");
    println!("cargo:rerun-if-changed=../../schemas");

    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("runtime-server manifest directory"),
    );
    let workspace_root = manifest_dir.join("../..");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
        .join("runtime-schema-bundles.v1.json");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("schema catalog build runtime");
    runtime
        .block_on(
            agent_semantic_schema_manager::build_support::runtime_catalog::compile_runtime_schema_catalog(
                &workspace_root,
                &output,
            ),
        )
        .expect("compile immutable Runtime schema catalog");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/asp-provider-stream.proto",
                "proto/asp-python-graphs.proto",
            ],
            &["proto"],
        )
        .expect("compile runtime-server gRPC protocols");
}
