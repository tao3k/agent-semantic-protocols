use crate::provider_command::facade::document::org::commands::agent_session::support::{
    write_codex_asp_explorer_fixture_with_actual_agent_path,
    write_codex_asp_explorer_fixture_with_actual_profile,
    write_codex_asp_explorer_fixture_with_default_agent_role,
    write_codex_asp_explorer_fixture_without_agent_path,
};
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_switch_model_updates_codex_dynamic_config_and_agent_projection() {
    let root = temp_project_root("agent-command-session-codex-switch-model");
    let home = root.join("home");
    let asp_agents_dir = home.join(".agent-semantic-protocols").join("agents");
    std::fs::create_dir_all(&asp_agents_dir).expect("create ASP agents dir");
    std::fs::write(
        asp_agents_dir.join("config.toml"),
        r#"[platform.codex.models]
primary = "gpt-5.3-codex-spark"
fallback = ["gpt-5.4-mini"]
capacityThreshold = 0.8
"#,
    )
    .expect("write ASP agents config");
    std::fs::write(
        asp_agents_dir.join("asp-explorer_codex.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write ASP explorer config");
    std::fs::write(
        asp_agents_dir.join("asp-testing_codex.toml"),
        "name = \"asp_testing\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write ASP testing config");

    let codex_agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&codex_agents_dir).expect("create Codex agents dir");
    std::fs::write(
        codex_agents_dir.join("asp-explorer.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write projected ASP explorer config");
    std::fs::write(
        codex_agents_dir.join("unrelated.toml"),
        "name = \"unrelated\"\nmodel = \"gpt-5.3-codex-spark\"\n",
    )
    .expect("write unrelated Codex config");

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .args([
            "agent",
            "session",
            "switch-model",
            "--model",
            "gpt-5.4-mini",
            "--json",
        ])
        .output()
        .expect("run asp agent session switch-model");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.contains("\"status\":\"switched\""), "{stdout}");
    assert!(stdout.contains("\"platform\":\"codex\""), "{stdout}");

    let dynamic_config =
        std::fs::read_to_string(asp_agents_dir.join("config.toml")).expect("read dynamic config");
    assert!(dynamic_config.contains("primary = \"gpt-5.4-mini\""));
    assert!(dynamic_config.contains("fallback = [\"gpt-5.4-mini\"]"));
    let asp_explorer = std::fs::read_to_string(asp_agents_dir.join("asp-explorer_codex.toml"))
        .expect("read ASP explorer config");
    assert!(asp_explorer.contains("model = \"gpt-5.4-mini\""));
    let asp_testing = std::fs::read_to_string(asp_agents_dir.join("asp-testing_codex.toml"))
        .expect("read ASP testing config");
    assert!(asp_testing.contains("model = \"gpt-5.4-mini\""));
    let codex_explorer = std::fs::read_to_string(codex_agents_dir.join("asp-explorer.toml"))
        .expect("read projected ASP explorer config");
    assert!(codex_explorer.contains("model = \"gpt-5.4-mini\""));
    let codex_testing = std::fs::read_to_string(codex_agents_dir.join("asp-testing.toml"))
        .expect("read projected ASP testing config");
    assert!(codex_testing.contains("model = \"gpt-5.4-mini\""));
    let unrelated =
        std::fs::read_to_string(codex_agents_dir.join("unrelated.toml")).expect("read unrelated");
    assert!(unrelated.contains("model = \"gpt-5.3-codex-spark\""));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_switch_model_can_target_one_resident_agent_config() {
    let root = temp_project_root("agent-command-session-codex-switch-model-one-agent");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_with_actual_profile(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.4-mini",
        "read-only",
        "read-only",
    );
    let asp_agents_dir = home.join(".agent-semantic-protocols").join("agents");
    std::fs::create_dir_all(&asp_agents_dir).expect("create ASP agents dir");
    std::fs::write(
        asp_agents_dir.join("config.toml"),
        r#"[platform.codex.models]
primary = "gpt-5.3-codex-spark"
fallback = ["gpt-5.4-mini"]
capacityThreshold = 0.8

[agents.asp_explorer]
session_name = "asp-explore"
host_agent_name = "asp_explorer"
profile = "asp-explorer_codex.toml"
projection = "asp-explorer.toml"
session_lifetime = "resident"
roles = ["subagent", "search"]
permissions = ["read-only"]
sandbox_mode = "read-only"

[agents.asp_testing]
session_name = "asp-testing"
host_agent_name = "asp_testing"
profile = "asp-testing_codex.toml"
projection = "asp-testing.toml"
session_lifetime = "resident"
roles = ["subagent", "testing", "build"]
permissions = ["workspace-write"]
sandbox_mode = "workspace-write"
"#,
    )
    .expect("write ASP agents config");
    std::fs::write(
        asp_agents_dir.join("asp-explorer_codex.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write ASP explorer config");
    std::fs::write(
        asp_agents_dir.join("asp-testing_codex.toml"),
        "name = \"asp_testing\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"workspace-write\"\n",
    )
    .expect("write ASP testing config");

    let codex_agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&codex_agents_dir).expect("create Codex agents dir");
    std::fs::write(
        codex_agents_dir.join("asp-explorer.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write projected ASP explorer config");
    std::fs::write(
        codex_agents_dir.join("asp-testing.toml"),
        "name = \"asp_testing\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"workspace-write\"\n",
    )
    .expect("write projected ASP testing config");

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .args([
            "agent",
            "session",
            "switch-model",
            "--name",
            "asp-explore",
            "--model",
            "gpt-5.4-mini",
            "--json",
        ])
        .output()
        .expect("run asp agent session switch-model");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stdout.contains("\"scope\":\"session:asp-explore\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"semantics\":\"configuration-layer\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("this command does not change the main session model"),
        "{stdout}"
    );

    let dynamic_config =
        std::fs::read_to_string(asp_agents_dir.join("config.toml")).expect("read dynamic config");
    assert!(dynamic_config.contains("primary = \"gpt-5.3-codex-spark\""));
    assert!(dynamic_config.contains("model = \"gpt-5.4-mini\""));
    let asp_explorer = std::fs::read_to_string(asp_agents_dir.join("asp-explorer_codex.toml"))
        .expect("read ASP explorer config");
    assert!(asp_explorer.contains("model = \"gpt-5.4-mini\""));
    let asp_testing = std::fs::read_to_string(asp_agents_dir.join("asp-testing_codex.toml"))
        .expect("read ASP testing config");
    assert!(asp_testing.contains("model = \"gpt-5.3-codex-spark\""));
    let codex_explorer = std::fs::read_to_string(codex_agents_dir.join("asp-explorer.toml"))
        .expect("read projected ASP explorer config");
    assert!(codex_explorer.contains("model = \"gpt-5.4-mini\""));
    let codex_testing = std::fs::read_to_string(codex_agents_dir.join("asp-testing.toml"))
        .expect("read projected ASP testing config");
    assert!(codex_testing.contains("model = \"gpt-5.3-codex-spark\""));

    let register_output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "codex-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--message-target-id",
            "codex-child-thread",
            "--roles",
            "subagent,search",
            "--model",
            "gpt-5.4-mini",
            "--json",
        ])
        .output()
        .expect("register ASP explorer after targeted switch");
    let register_stdout = String::from_utf8_lossy(&register_output.stdout);
    let register_stderr = String::from_utf8_lossy(&register_output.stderr);
    assert!(register_output.status.success(), "{register_stderr}");
    assert!(
        !register_stdout.contains("child-session model mismatch"),
        "{register_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}
