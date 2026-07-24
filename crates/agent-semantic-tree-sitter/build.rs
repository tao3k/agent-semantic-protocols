use asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env;

fn main() {
    assert_asp_rust_project_harness_member_policy_from_env(env!("CARGO_PKG_NAME"));
}
