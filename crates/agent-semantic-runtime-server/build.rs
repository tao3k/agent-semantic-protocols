fn main() {
    println!("cargo:rerun-if-changed=proto/asp-provider-stream.proto");
    println!("cargo:rerun-if-changed=proto/asp-python-graphs.proto");
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
