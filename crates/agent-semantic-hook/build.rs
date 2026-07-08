fn main() {
    asp_rust_project_harness_policy::build_gate::assert_asp_rust_project_harness_member_policy_from_env(
        env!("CARGO_PKG_NAME"),
    );
}
