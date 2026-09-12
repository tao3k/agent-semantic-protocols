// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[cfg(feature = "grpc-session")]
fn main() {
    let _policy_receipt =
        asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env();
    println!(
        "cargo:rerun-if-changed=../agent-semantic-runtime-server/proto/asp-provider-stream.proto"
    );
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("resolve the feature-owned vendored protoc binary");
    let mut prost_config = tonic_prost_build::Config::new();
    prost_config.protoc_executable(protoc);
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_with_config(
            prost_config,
            &["../agent-semantic-runtime-server/proto/asp-provider-stream.proto"],
            &["../agent-semantic-runtime-server/proto"],
        )
        .expect("compile provider-transport provider stream proto");
}

#[cfg(not(feature = "grpc-session"))]
fn main() {}
