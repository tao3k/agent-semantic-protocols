// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

fn main() {
    let _policy_receipt =
        asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env();
    let project_root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("symbol index CARGO_MANIFEST_DIR"),
    );
    let out_dir =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("symbol index OUT_DIR"));
    let cargo_build_root = out_dir
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "build"))
        .expect("symbol index Cargo build boundary");
    let canonical_root = project_root
        .canonicalize()
        .expect("canonical symbol index project root");
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent.semantic-protocols.asp-rust-member-cache.v1\0");
    identity.update(canonical_root.as_os_str().as_encoded_bytes());
    let identity = identity.finalize().to_hex();
    let cache_root = cargo_build_root
        .join(".asp-rust-project-harness-policy")
        .join("v1")
        .join(&identity[..32]);
    let config = asp_rust::default_asp_rust_config()
        .with_cargo_check_advice_allow_explanation("scope=agent-semantic-symbol-index cargo-check advice; owner=language-neutral symbol skeleton index gate; finding_category=advisory policy findings; why_safe_now=the isolated index keeps advisory findings visible while warning and error findings fail the build; cleanup_trigger=clear the symbol index advisory backlog and remove this allowance")
        .with_latency_sensitive_performance_owner(
            "src/symbol_skeleton_index.rs",
            "rarest-first cross-language symbol posting intersection is a resident Search hot path",
        );
    let policy = asp_rust::AspRustDownstreamPolicy::new(
        "agent-semantic-protocols::agent-semantic-symbol-index",
        config,
    );
    let authority = asp_rust::AspRustBuildGateAuthority::new(
        cache_root,
        asp_rust_project_harness_policy::asp_workspace_member_policies()
            .iter()
            .find(|policy| policy.package_name == "agent-semantic-symbol-index")
            .expect("registered symbol index policy")
            .contract_digest(),
    )
    .expect("symbol index ASP Rust cache authority");
    let _style_receipt = asp_rust::assert_asp_rust_downstream_policy_with_authority(
        &project_root,
        &policy,
        &authority,
    );
}
