use asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env;

fn main() {
    println!("cargo:rerun-if-changed=../../languages/org/contracts/asp.skill.v1.org");
    println!("cargo:rerun-if-changed=../agent-semantic-runtime/src/codex_app_server_sessions.rs");
    assert_asp_rust_project_harness_member_policy_from_env(env!("CARGO_PKG_NAME"));
}
