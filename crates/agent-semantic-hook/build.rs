use asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env;
use std::path::PathBuf;

fn main() {
    assert_asp_rust_project_harness_member_policy_from_env(env!("CARGO_PKG_NAME"));
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let resolved =
        asp_provider_registry_build::resolve_provider_register(manifest_dir.join("../.."))
            .expect("resolve provider register");
    for path in resolved.input_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let agents_root = manifest_dir.join("../../agents");
    println!("cargo:rerun-if-changed={}", agents_root.display());
    let projection =
        agent_semantic_config::agent_route_registry::render_hook_agent_routes(&agents_root)
            .expect("render hook agent routes from agents registry and platform projections");
    let output_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir"));
    std::fs::write(output_dir.join("hook-agent-routes.toml"), projection)
        .expect("write hook agent route projection");
}
