fn main() {
    asp_rust_project_harness_policy::build_gate::assert_asp_rust_project_harness_member_policy_from_env(
        env!("CARGO_PKG_NAME"),
    );
    println!("cargo:rerun-if-changed=proto/asp-client-protocol.proto");
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/asp-client-protocol.proto"], &["proto"])
        .expect("compile public ASP Client Protocol gRPC transport");
}
