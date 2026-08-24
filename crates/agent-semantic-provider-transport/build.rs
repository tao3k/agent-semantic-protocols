fn main() {
    println!(
        "cargo:rerun-if-changed=../agent-semantic-runtime-server/proto/asp-provider-stream.proto"
    );
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(
            &["../agent-semantic-runtime-server/proto/asp-provider-stream.proto"],
            &["../agent-semantic-runtime-server/proto"],
        )
        .expect("compile provider-transport provider stream proto");
}
