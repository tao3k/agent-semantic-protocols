// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

fn main() {
    asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env();

    let manifest =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = manifest.join("../..");
    let resolved =
        agent_semantic_schema_manager::build_support::provider_registry::resolve_provider_register(
            &root,
        )
        .expect("resolve provider register");
    for path in &resolved.input_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir"))
        .join("provider-register.resolved.json");
    std::fs::write(out, resolved.bytes).expect("write resolved provider register");
}
