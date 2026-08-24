fn main() {
    println!("cargo:rerun-if-changed=proto/asp-provider-stream.proto");
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["proto/asp-provider-stream.proto"], &["proto"])
        .expect("compile runtime-server provider stream proto");
}
