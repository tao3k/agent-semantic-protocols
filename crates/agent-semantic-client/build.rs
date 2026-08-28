use asp_rust_project_harness_policy::assert_asp_rust_project_harness_member_policy_from_env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../../org/contracts/asp.skill.v1.org");
    println!(
        "cargo:rerun-if-changed=../../org/contracts/agent.multi-agent-session-control-plane.v1.org"
    );
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let agents_root = manifest_dir.join("../../agents");
    println!("cargo:rerun-if-changed={}", agents_root.display());
    let registry = agent_semantic_config::agent_route_registry::load_agent_route_registry(
        &agents_root.join("config.toml"),
    )
    .expect("load canonical agent route registry");
    let mut explore_routes = registry.registry.agents.iter().filter(|(_, route)| {
        route.session_lifetime
            == agent_semantic_config::agent_route_registry::AgentSessionLifetime::Resident
            && route.roles.iter().any(|role| role == "explore")
    });
    let (route_key, _) = explore_routes.next().expect("one resident explore route");
    assert!(
        explore_routes.next().is_none(),
        "agent registry must define exactly one resident explore route"
    );
    let route = agent_semantic_config::agent_route_registry::compile_agent_route(
        &registry, route_key, "codex",
    )
    .expect("compile canonical Codex explore route");
    let model = route
        .model
        .expect("Codex explore projection requires model");
    let output_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir"));
    std::fs::write(output_dir.join("codex-explore-default-model.txt"), model)
        .expect("write compiled Codex explore model");
    assert_asp_rust_project_harness_member_policy_from_env(env!("CARGO_PKG_NAME"));
}
