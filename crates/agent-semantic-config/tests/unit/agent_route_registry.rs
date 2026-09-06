// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::AgentSessionLifetime;
use super::compile_agent_route;
use super::load_agent_route_registry;
use super::load_agent_route_registry_for_platform;

fn canonical_registry_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agents/config.toml")
}

fn canonical_agent_prompt(file_name: &str) -> String {
    let path = canonical_registry_path()
        .parent()
        .expect("agent registry directory")
        .join(file_name);
    let source = std::fs::read_to_string(&path).expect("read canonical agent prompt");
    if file_name.ends_with("_codex.toml") {
        return toml::from_str::<toml::Value>(&source)
            .expect("parse Codex agent projection")
            .get("developer_instructions")
            .and_then(toml::Value::as_str)
            .expect("Codex developer instructions")
            .to_owned();
    }

    source
        .splitn(3, "---")
        .nth(2)
        .expect("Claude agent prompt body")
        .trim()
        .to_owned()
}

#[test]
fn canonical_agent_prompts_contain_only_role_boundary_and_playbook_flow() {
    const PROMPTS: [&str; 6] = [
        "asp_explorer_codex.toml",
        "asp_explorer_claude.md",
        "asp_testing_codex.toml",
        "asp_testing_claude.md",
        "asp_coding_codex.toml",
        "asp_coding_claude.md",
    ];
    const IMPLEMENTATION_LEAKS: [&str; 8] = [
        "schemaVersion",
        "schemaId",
        "asp.search.playbook-receipt",
        "seed products",
        "hookMatcherGeneration",
        "ASP_NO_AGENT",
        "/root/",
        "agent.semantic-protocols",
    ];

    for file_name in PROMPTS {
        let prompt = canonical_agent_prompt(file_name);
        assert!(prompt.contains("Role:"), "{file_name} requires a role");
        assert!(
            prompt.contains("Playbook:"),
            "{file_name} requires a Playbook flow"
        );
        for leak in IMPLEMENTATION_LEAKS {
            assert!(
                !prompt.contains(leak),
                "{file_name} leaks implementation detail `{leak}`"
            );
        }
    }
}

#[test]
fn explorer_host_projections_carry_selector_only_output_guidance() {
    for file_name in ["asp_explorer_codex.toml", "asp_explorer_claude.md"] {
        let prompt = canonical_agent_prompt(file_name);
        for required in [
            "Output guidance:",
            "at most three ranked evidence entries",
            "Never return source text, snippets, excerpts",
            "Do not read source merely to summarize it",
            "Do not prescribe a next command",
            "The parent reasons from the returned evidence",
        ] {
            assert!(
                prompt.contains(required),
                "{file_name} omits selector-only output guidance `{required}`"
            );
        }
    }
}

#[test]
fn codex_worker_and_default_roles_resolve_only_to_owner_scoped_coding() {
    let loaded =
        load_agent_route_registry(&canonical_registry_path()).expect("canonical agent registry");

    for host_role in ["worker", "default"] {
        let coding = loaded
            .compile_route_for_platform_host_role("codex", host_role)
            .expect("Codex role lookup")
            .expect("configured coding role");
        assert_eq!(coding.route_key.as_str(), "asp_coding");
        assert_eq!(coding.platform_host_agent_name.as_str(), "asp_coding");
        assert_eq!(coding.allowed_rule_intents, ["owner-scoped-mutation"]);
        assert_eq!(
            coding.sandbox_mode.map(|mode| mode.as_str()),
            Some("workspace-write")
        );
    }
}

#[test]
fn embedded_registry_preserves_native_role_resolution() {
    let state_home = tempfile::tempdir().expect("temporary embedded agent state");
    for asset in crate::embedded_agent_assets::embedded_agent_assets() {
        std::fs::write(state_home.path().join(asset.file_name), asset.contents)
            .expect("materialize embedded agent asset");
    }

    let registry = load_agent_route_registry(&state_home.path().join("config.toml"))
        .expect("load embedded agent registry");
    let native = registry
        .compile_route_for_platform_host_identity("codex", "explorer")
        .expect("resolve native explorer role")
        .expect("native explorer route");
    let canonical = registry
        .compile_route_for_platform_host_identity("codex", "asp_explorer")
        .expect("resolve canonical explorer name")
        .expect("canonical explorer route");

    assert_eq!(native.route_key.as_str(), "asp_explorer");
    assert_eq!(native, canonical);
}

#[test]
fn canonical_registry_compiles_host_routes() {
    let loaded = load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
    let explorer_role = loaded
        .compile_route_for_platform_host_role("codex", "explorer")
        .expect("Codex explorer role lookup")
        .expect("configured explorer role");
    assert_eq!(explorer_role.route_key.as_str(), "asp_explorer");
    assert_eq!(
        explorer_role.platform_host_agent_name.as_str(),
        "asp_explorer"
    );
    assert_eq!(
        explorer_role.allowed_rule_intents,
        ["reasoning-search", "structured-projection"]
    );
    assert_eq!(
        explorer_role.sandbox_mode.map(|mode| mode.as_str()),
        Some("read-only")
    );
    let testing_role = loaded
        .compile_route_for_platform_host_role("codex", "testing")
        .expect("Codex testing role lookup")
        .expect("configured testing role");
    assert_eq!(testing_role.route_key.as_str(), "asp_testing");
    assert_eq!(
        testing_role.platform_host_agent_name.as_str(),
        "asp_testing"
    );
    assert_eq!(
        loaded
            .compile_route_for_platform_host_identity("codex", "explorer")
            .expect("native explorer identity")
            .expect("configured explorer identity")
            .route_key
            .as_str(),
        "asp_explorer"
    );
    assert_eq!(
        loaded
            .compile_route_for_platform_host_identity("codex", "asp_testing")
            .expect("canonical testing identity")
            .expect("configured testing identity")
            .route_key
            .as_str(),
        "asp_testing"
    );
    assert!(
        loaded
            .compile_route_for_platform_host_role("codex", "unknown-role")
            .expect("unknown role lookup")
            .is_none()
    );
    assert!(
        loaded
            .compile_route_for_platform_host_role("unknown-platform", "explorer")
            .expect("cross-platform role lookup")
            .is_none()
    );
    let ambiguous = loaded
        .compile_route_for_platform_host_role("codex", "subagent")
        .expect_err("shared generic role must fail closed at resolution");
    assert!(ambiguous.contains("agent role `subagent` is ambiguous"));
    assert_eq!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "asp_explorer")
            .expect("compiled session lookup")
            .map(|route| route.route_key.0),
        Some("asp_explorer".to_owned())
    );
    assert!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "asp-explore")
            .expect("exact missing session lookup")
            .is_none(),
        "legacy punctuation aliases must not resolve"
    );
    let codex = compile_agent_route(&loaded, "asp_explorer", "codex").expect("Codex route");
    let claude = compile_agent_route(&loaded, "asp_explorer", "claude").expect("Claude route");
    assert_eq!(codex.route_key.as_str(), "asp_explorer");
    assert_eq!(claude.route_key.as_str(), "asp_explorer");
    assert_eq!(codex.session_lifetime, AgentSessionLifetime::Resident);
    assert!(codex.is_resident_agent());
    assert_eq!(codex.focus_mode, super::AgentFocusMode::Leaf);
    assert_eq!(codex.platform.as_str(), "codex");
    assert_eq!(claude.platform.as_str(), "claude");
    assert_eq!(codex.platform_host_agent_name.as_str(), "asp_explorer");
    assert_eq!(claude.platform_host_agent_name.as_str(), "asp-explorer");
    assert_eq!(
        codex.host_invocation("@asp_explorer").symbol,
        "@asp_explorer"
    );
    assert_eq!(
        codex.host_invocation("@asp_explorer").syntax,
        "@asp_explorer"
    );
    assert_eq!(
        claude.host_invocation("@agent-asp-explorer").symbol,
        "@agent-asp-explorer"
    );
    assert_eq!(
        claude.host_invocation("@agent-asp-explorer").syntax,
        "@agent-asp-explorer"
    );
    assert_eq!(codex.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(claude.model.as_deref(), Some("haiku"));
    assert!(codex.profile_path.ends_with("asp_explorer_codex.toml"));
    assert!(claude.profile_path.ends_with("asp_explorer_claude.md"));
    assert_eq!(codex.roles, vec!["explore", "explorer", "subagent"]);
    assert_eq!(
        codex.allowed_rule_intents,
        vec!["reasoning-search", "structured-projection"]
    );
    assert_eq!(codex.agent_kind.as_str(), "agent");
    assert_eq!(codex.display_role, "Evidence Explorer");
    assert_eq!(codex.description, "for code and evidence search");
    assert_eq!(claude.description, codex.description);
    assert_eq!(
        codex.sandbox_mode.map(|mode| mode.as_str()),
        Some("read-only")
    );
    assert!(
        codex
            .effective_permissions
            .denies(super::AgentPermissionAction::Edit)
    );
    assert!(
        claude
            .effective_permissions
            .denies(super::AgentPermissionAction::Edit)
    );
    assert_eq!(
        claude.definition_schema_id,
        super::ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA_ID
    );
    let testing =
        compile_agent_route(&loaded, "asp_testing", "codex").expect("Codex testing route");
    assert_eq!(testing.platform_host_agent_name.as_str(), "asp_testing");
    assert_eq!(testing.focus_mode, super::AgentFocusMode::Leaf);
    assert_eq!(testing.agent_kind.as_str(), "agent");
    assert!(testing.is_resident_agent());
    assert_eq!(testing.display_role, "Test Runner");
    assert_eq!(
        testing.allowed_rule_intents,
        vec![
            "test-build-command",
            "review-command",
            "live-corpus-qualification",
            "git-history-inspection"
        ]
    );
    assert_eq!(testing.description, "for build and test jobs");
    assert!(testing.profile_path.ends_with("asp_testing_codex.toml"));
    assert_eq!(
        testing.sandbox_mode.map(|mode| mode.as_str()),
        Some("read-only")
    );
    let coding = compile_agent_route(&loaded, "asp_coding", "codex").expect("Codex coding route");
    assert_eq!(coding.platform_host_agent_name.as_str(), "asp_coding");
    assert_eq!(coding.focus_mode, super::AgentFocusMode::Leaf);
    assert_eq!(coding.agent_kind.as_str(), "agent");
    assert!(coding.is_resident_agent());
    assert_eq!(coding.display_role, "Owner-scoped Coding Worker");
    assert_eq!(
        coding.roles,
        vec!["coding", "default", "subagent", "worker"]
    );
    assert_eq!(coding.allowed_rule_intents, vec!["owner-scoped-mutation"]);
    assert_eq!(
        coding.description,
        "for edits restricted to explicitly registered owner paths"
    );
    assert!(coding.profile_path.ends_with("asp_coding_codex.toml"));
    assert_eq!(
        coding.sandbox_mode.map(|mode| mode.as_str()),
        Some("workspace-write")
    );
    assert!(
        !coding
            .effective_permissions
            .denies(super::AgentPermissionAction::Edit)
    );
    let claude_testing =
        compile_agent_route(&loaded, "asp_testing", "claude").expect("Claude testing route");
    assert_eq!(
        claude_testing.platform_host_agent_name.as_str(),
        "asp-testing"
    );
    assert!(
        claude_testing
            .effective_permissions
            .denies(super::AgentPermissionAction::Edit)
    );
}

#[test]
fn temporary_subagent_route_is_never_a_resident_db_agent() {
    let root = tempfile::tempdir().expect("temporary Agent Loader root");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[agents.temporary_probe]
session_lifetime = "temporary"
roles = ["temporary-probe"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "subagent"
display_role = "Temporary Probe"
description = "temporary Host subagent"
"#,
    )
    .expect("write temporary registry");
    std::fs::write(
        root.path().join("temporary_probe_codex.toml"),
        r#"name = "temporary_probe"
description = "temporary Host subagent"
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Return a bounded receipt."
"#,
    )
    .expect("write temporary Host profile");

    let loaded = load_agent_route_registry_for_platform(&root.path().join("config.toml"), "codex")
        .expect("load temporary Agent registry");
    let route = loaded
        .compile_route_for_platform_host_identity("codex", "temporary_probe")
        .expect("resolve temporary Host identity")
        .expect("temporary route exists");

    assert_eq!(route.agent_kind.as_str(), "subagent");
    assert_eq!(route.session_lifetime, AgentSessionLifetime::Temporary);
    assert!(!route.is_resident_agent());
}

#[test]
fn managed_agent_schemas_have_stable_identity_and_internal_version() {
    for (schema, expected_id) in [
        (
            super::CODEX_AGENT_DEFINITION_SCHEMA,
            super::CODEX_AGENT_DEFINITION_SCHEMA_ID,
        ),
        (
            super::ANTHROPIC_AGENT_FRONTMATTER_SCHEMA,
            super::ANTHROPIC_AGENT_FRONTMATTER_SCHEMA_ID,
        ),
        (
            super::ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA,
            super::ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA_ID,
        ),
        (
            super::AGENT_ROUTE_REGISTRY_SCHEMA,
            super::AGENT_ROUTE_REGISTRY_DOCUMENT_SCHEMA_ID,
        ),
    ] {
        assert!(schema.contains(&format!("\"$id\": \"{expected_id}\"")));
        assert!(schema.contains("\"x-asp-schema-version\": 1"));
        assert!(
            !expected_id.contains(":v1")
                && !expected_id.contains(":v2")
                && !expected_id.contains(":v3")
        );
    }
}

#[test]
fn route_registry_schema_owns_the_closed_agent_kind_set() {
    let schema: serde_json::Value =
        serde_json::from_str(super::AGENT_ROUTE_REGISTRY_SCHEMA).expect("route registry schema");
    assert_eq!(
        schema["$defs"]["agentRoute"]["properties"]["agent_kind"]["enum"],
        serde_json::json!(["agent", "subagent"])
    );
}

#[test]
fn route_registry_rejects_an_agent_kind_outside_the_schema_set() {
    let error = super::parse_agent_route_registry(
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1
[platforms.codex]
matcher = "*_codex.toml"
[agents.probe]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "worker"
display_role = "Probe"
"#,
        "invalid agent kind fixture",
    )
    .expect_err("agent_kind outside the schema enum must fail closed");
    assert!(error.contains("unknown variant `worker`"), "{error}");
}

#[test]
fn anthropic_permission_modes_match_the_official_frontmatter_contract() {
    let schema: serde_json::Value = serde_json::from_str(super::ANTHROPIC_AGENT_FRONTMATTER_SCHEMA)
        .expect("parse Anthropic agent schema");
    assert_eq!(
        schema["properties"]["permissionMode"]["enum"],
        serde_json::json!([
            "default",
            "acceptEdits",
            "auto",
            "dontAsk",
            "bypassPermissions",
            "plan"
        ])
    );
}

#[test]
fn unknown_platform_matcher_fields_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("config.toml");
    std::fs::write(
        &path,
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"
manager_projection = "legacy-shadow"

[agents.bad]
session_lifetime = "resident"
roles = ["subagent"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "agent"
"#,
    )
    .expect("write invalid route registry");
    let error = load_agent_route_registry(&path)
        .expect_err("unknown platform matcher field must fail closed");
    assert!(error.contains("unknown field `manager_projection`"));
}

#[test]
fn unknown_platform_fails_closed() {
    let loaded = load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
    let error = compile_agent_route(&loaded, "asp_testing", "unknown-host")
        .expect_err("unknown platform must fail closed");
    assert!(error.contains("does not define `asp_testing` for `unknown-host`"));
}

#[test]
fn platform_matcher_builds_distinct_native_name_registries() {
    let root = tempfile::tempdir().expect("platform registry tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"
[platforms.claude]
matcher = "*_claude.md"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "agent"
"#,
    )
    .expect("write matcher registry");
    std::fs::write(
        root.path().join("custom_codex.toml"),
        r#"name = "codex_probe"
description = "Codex evidence probe."
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run Codex evidence queries."
"#,
    )
    .expect("write Codex matcher profile");
    std::fs::write(
        root.path().join("custom_claude.md"),
        r#"---
name: claude-probe
description: Claude evidence probe.
model: haiku
---
Run Claude evidence queries.
"#,
    )
    .expect("write Claude matcher profile");

    let loaded = load_agent_route_registry(&root.path().join("config.toml"))
        .expect("load platform registries");
    assert_eq!(
        compile_agent_route(&loaded, "custom", "codex")
            .expect("Codex route")
            .platform_host_agent_name
            .as_str(),
        "codex_probe"
    );
    assert_eq!(
        compile_agent_route(&loaded, "custom", "claude")
            .expect("Claude route")
            .platform_host_agent_name
            .as_str(),
        "claude-probe"
    );
    assert!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "claude-probe")
            .expect("platform exact lookup")
            .is_none()
    );
}

#[test]
fn codex_projection_rejects_unknown_fields() {
    let root = tempfile::tempdir().expect("Codex projection tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "agent"
"#,
    )
    .expect("write route registry");
    std::fs::write(
        root.path().join("custom_codex.toml"),
        r#"name = "codex_probe"
description = "Codex evidence probe."
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run Codex evidence queries."
permission_mode = "compatibility-shadow"
"#,
    )
    .expect("write invalid Codex projection");

    let error = load_agent_route_registry(&root.path().join("config.toml"))
        .expect_err("unknown Codex fields must fail closed");
    assert!(error.contains("unknown `permission_mode` field"));
}

#[test]
fn claude_plugin_projection_rejects_ignored_permission_fields() {
    let root = tempfile::tempdir().expect("Claude projection tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.claude]
matcher = "*_claude.md"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "agent"
"#,
    )
    .expect("write route registry");
    std::fs::write(
        root.path().join("custom_claude.md"),
        r#"---
name: claude-probe
description: Claude evidence probe.
model: haiku
permissionMode: plan
---
Run Claude evidence queries.
"#,
    )
    .expect("write invalid Claude plugin projection");

    let error = load_agent_route_registry(&root.path().join("config.toml"))
        .expect_err("ignored Claude plugin permission fields must fail closed");
    assert!(error.contains("unknown or ignored `permissionMode` frontmatter"));
}

#[test]
fn platform_scoped_load_does_not_hydrate_unrelated_host_artifacts() {
    let root = tempfile::tempdir().expect("platform isolation tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[platforms.claude]
matcher = "*_claude.md"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "agent"
"#,
    )
    .expect("write route registry");
    std::fs::write(
        root.path().join("custom_codex.toml"),
        r#"name = "codex_probe"
description = "Codex evidence probe."
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run Codex evidence queries."
"#,
    )
    .expect("write Codex projection");
    std::fs::write(
        root.path().join("custom_claude.md"),
        r#"---
name: claude-probe
description: Claude evidence probe.
permissionMode: plan
---
Run Claude evidence queries.
"#,
    )
    .expect("write stale Claude projection");

    let loaded = load_agent_route_registry_for_platform(&root.path().join("config.toml"), "codex")
        .expect("Codex load must be isolated from Claude artifacts");
    assert!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "codex_probe")
            .expect("Codex route lookup")
            .is_some()
    );
    assert!(
        load_agent_route_registry(&root.path().join("config.toml"))
            .expect_err("full publication gate must still validate Claude")
            .contains("permissionMode")
    );
}

#[test]
fn platform_matcher_rejects_noncanonical_globs() {
    let error = super::parse_agent_route_registry(
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1
[platforms.codex]
matcher = "*.toml"
[agents.probe]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
agent_kind = "agent"
"#,
        "noncanonical matcher fixture",
    )
    .expect_err("broad platform matcher must fail closed");
    assert!(error.contains("must be exactly `*_codex.toml`"));
}
