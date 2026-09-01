#[cfg(feature = "compiler")]
use std::path::PathBuf;

#[cfg(any(feature = "compiler", feature = "evaluator"))]
mod reader_probe;

#[cfg(feature = "compiler")]
fn main() {
    assert_member_policy();
    reader_probe::compile_reader_probe_artifacts();
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let resolved =
        agent_semantic_schema_manager::build_support::provider_registry::resolve_provider_register(
            manifest_dir.join("../.."),
        )
        .expect("resolve provider register");
    for path in resolved.input_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

#[cfg(all(feature = "evaluator", not(feature = "compiler")))]
fn main() {
    assert_member_policy();
    reader_probe::compile_reader_probe_artifacts();
}

#[cfg(not(any(feature = "compiler", feature = "evaluator")))]
fn main() {
    assert_member_policy();
}

fn assert_member_policy() {
    asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env(env!(
        "CARGO_PKG_NAME"
    ));
}
