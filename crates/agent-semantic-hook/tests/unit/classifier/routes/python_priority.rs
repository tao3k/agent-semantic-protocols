use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::{ProviderFixtureLayout, provider, registry_with_python};

fn assert_python_direct_read(payload: serde_json::Value) {
    let decision = classify_hook(&registry_with_python(), "codex", "pre-tool", &payload);
    assert_eq!(decision.decision, DecisionKind::Deny, "{payload}");
    assert_eq!(
        decision.reason_kind,
        ReasonKind::DirectSourceRead,
        "{payload}"
    );
    assert_eq!(decision.language_ids, ["python"], "{payload}");
}

#[test]
fn python_registered_extensions_cover_codex_read_path_field_shapes() {
    for field in [
        "path",
        "file",
        "file_path",
        "filePath",
        "absolutePath",
        "relativePath",
        "uri",
    ] {
        assert_python_direct_read(json!({
            "toolName": "Read",
            "toolInput": {field: "src/tools/semantic_sandtable/receipt_reports.py"}
        }));
    }
}

#[test]
fn python_registered_extensions_cover_json_string_and_nested_command_shapes() {
    assert_python_direct_read(json!({
        "toolName": "Read",
        "toolInput": "{\"filePath\":\"src/tools/semantic_sandtable/receipt_reports.py\"}"
    }));
    assert_python_direct_read(json!({
        "toolName": "functions.exec",
        "toolInput": {"commandActions": [{"toolName": "Read", "toolInput": {"path": "src/tools/semantic_sandtable/receipt_reports.py"}}]}
    }));
}

#[test]
fn programming_provider_manifests_cover_compact_and_nested_native_reads() {
    let providers = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .filter_map(|manifest| {
            let descriptor = manifest.project_resolution()?;
            let source_extensions = descriptor.source_extensions.clone();
            let config_files = descriptor.entry_markers.clone();
            let routes = agent_semantic_hook::materialize_provider_routes(&manifest)
                .expect("provider routes");
            let mut provider = provider(
                &manifest,
                ProviderFixtureLayout {
                    source_extensions: &[],
                    config_files: &[],
                },
                routes,
            );
            provider.source_extensions = source_extensions;
            provider.config_files = config_files;
            Some(provider)
        })
        .collect::<Vec<_>>();
    assert!(!providers.is_empty());
    for provider in &providers {
        for extension in &provider.source_extensions {
            let path = format!("src/witness{extension}");
            let runtime = agent_semantic_hook::HookRuntime {
                rankers: Vec::new(),
                project_root: ".".to_string(),
                providers: vec![provider.clone()],
                policy_providers: Vec::new(),
            };
            for tool_input in [
                json!({"type":"read", "path": path}),
                json!({"toolName":"Read", "toolInput":{"path": path}}),
            ] {
                let decision = classify_hook(
                    &runtime,
                    "codex",
                    "pre-tool",
                    &json!({"toolName":"functions.exec", "toolInput":{"commandActions":[tool_input]}}),
                );
                assert_eq!(
                    decision.decision,
                    DecisionKind::Deny,
                    "{} {path}",
                    provider.language_id
                );
                assert_eq!(
                    decision.reason_kind,
                    ReasonKind::DirectSourceRead,
                    "{} {path}",
                    provider.language_id
                );
                assert_eq!(
                    decision.language_ids,
                    [provider.language_id.as_str()],
                    "{} {path}",
                    provider.language_id
                );
            }
        }
    }
}

#[test]
fn namespaced_python_explicit_read_routes_to_owner_frontier() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "functions.read_file",
            "toolInput": {"path": "src/tools/semantic_sandtable/receipt_reports.py"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "asp-python");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "search",
            "owner",
            "tools/semantic_sandtable/receipt_reports.py",
            "items",
            "--query",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "--workspace",
            "src",
            "--view",
            "seeds"
        ]
    );
    assert!(
        !decision.routes[0]
            .argv
            .iter()
            .any(|arg| matches!(arg.as_str(), "query" | "--code" | "--content"))
    );
}

#[test]
fn exact_document_reads_route_to_owner_discovery_without_projection_flags() {
    for (path, language_id) in [("docs/guide.org", "org"), ("docs/guide.md", "md")] {
        let decision = classify_hook(
            &super::registry_with_documents(),
            "codex",
            "pre-tool",
            &json!({
                "toolName": "functions.read_file",
                "toolInput": {"path": path}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{path}");
        assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead, "{path}");
        assert_eq!(decision.language_ids, [language_id], "{path}");
        assert_eq!(decision.routes.len(), 1, "{path}");
        let route = &decision.routes[0];
        assert_eq!(route.kind, DecisionRouteKind::Owner, "{path}");
        assert_eq!(route.language_id, language_id, "{path}");
        assert_eq!(
            &route.argv[..4],
            ["asp", language_id, "search", "owner"],
            "{path}"
        );
        assert!(
            route.argv.iter().any(|arg| arg == path),
            "{path}: {:?}",
            route.argv
        );
        let path_index = route
            .argv
            .iter()
            .position(|arg| arg == path)
            .expect("document owner path");
        assert_eq!(
            route.argv.get(path_index + 1).map(String::as_str),
            Some("items"),
            "document owner recovery must materialize owner items for {path}: {:?}",
            route.argv
        );
        assert!(
            !route.argv.iter().any(|arg| matches!(
                arg.as_str(),
                "query" | "--content" | "--verbatim" | "--code"
            )),
            "{path}: {:?}",
            route.argv
        );
    }
}

#[test]
fn known_root_session_denies_direct_registered_rust_provider_binary() {
    let decision = classify_hook(
        &super::registry_with_documents(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "asp-rust query --selector example"},
            "session_id": "root-session-known"
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.reason_kind,
        ReasonKind::ProviderBinaryDirectExecution
    );
    assert_eq!(decision.language_ids, ["rust"]);
    assert_eq!(decision.routes.len(), 1);
    assert_eq!(decision.routes[0].binary, "asp");
    assert_eq!(&decision.routes[0].argv[..2], ["asp", "rust"]);
    assert_eq!(
        decision.fields["runtimeBinaryAdmissionDenial"],
        "missing-dispatch-capability"
    );
}

#[test]
fn known_root_session_denies_absolute_registered_runtime_artifact_path() {
    let decision = classify_hook(
        &super::registry_with_documents(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {
                "cmd": "/canonical/asp/runtime/bin/asp-rust query --selector example"
            },
            "session_id": "root-session-known"
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.reason_kind,
        ReasonKind::ProviderBinaryDirectExecution
    );
    assert_eq!(decision.routes[0].argv, ["asp", "rust"]);
}

#[test]
fn asp_facade_and_unknown_host_tool_are_not_provider_profile_denials() {
    for command in ["asp rust query --selector example", "git status --short"] {
        let decision = classify_hook(
            &super::registry_with_documents(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command},
                "session_id": "root-session-known"
            }),
        );

        assert_ne!(
            decision.reason_kind,
            ReasonKind::ProviderBinaryDirectExecution,
            "{command}"
        );
        assert!(
            decision
                .fields
                .get("runtimeBinaryAdmissionDenial")
                .is_none()
        );
    }
}

#[test]
fn plain_environment_string_cannot_bypass_missing_dispatch_capability() {
    let decision = classify_hook(
        &super::registry_with_documents(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {
                "cmd": "asp-rust query --selector example",
                "env": {
                    "ASP_RUNTIME_DISPATCH_CAPABILITY": "spoofed-unverified-string"
                }
            },
            "session_id": "root-session-known"
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.reason_kind,
        ReasonKind::ProviderBinaryDirectExecution
    );
    assert_eq!(
        decision.fields["runtimeBinaryAdmissionDenial"],
        "missing-dispatch-capability"
    );
}

#[test]
fn shell_environment_assignment_cannot_hide_registered_provider_binary() {
    let decision = classify_hook(
        &super::registry_with_documents(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {
                "cmd": "TRACE=1 MODE=audit asp-rust query --selector example"
            },
            "session_id": "root-session-known"
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.reason_kind,
        ReasonKind::ProviderBinaryDirectExecution
    );
    assert_eq!(decision.fields["runtimeBinary"], "asp-rust");
    assert_eq!(
        decision.fields["runtimeBinaryAdmissionDenial"],
        "missing-dispatch-capability"
    );
}

#[test]
fn python_pattern_read_routes_to_lexical_discovery() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "functions.read_file",
            "toolInput": {"path": "src/tools/**/*.py"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Lexical);
    assert_eq!(decision.routes[0].provider_id, "asp-python");
    assert!(
        !decision.routes[0]
            .argv
            .iter()
            .any(|arg| matches!(arg.as_str(), "query" | "--code" | "--content"))
    );
}

#[test]
fn python_embedded_read_text_routes_to_owner_frontier() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "python3 - <<'PY'\nfrom pathlib import Path\npath = Path('src/tools/semantic_sandtable/receipt_reports.py')\nprint(path.read_text())\nPY"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(
        decision.subject.command.as_deref(),
        Some(
            "python3 - <<'PY'\nfrom pathlib import Path\npath = Path('src/tools/semantic_sandtable/receipt_reports.py')\nprint(path.read_text())\nPY"
        )
    );
    assert_eq!(
        decision.subject.paths,
        ["src/tools/semantic_sandtable/receipt_reports.py"]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "asp-python");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "search",
            "owner",
            "tools/semantic_sandtable/receipt_reports.py",
            "items",
            "--query",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "--workspace",
            "src",
            "--view",
            "seeds"
        ]
    );
}

#[test]
fn python_nested_package_read_text_routes_to_provider_root() {
    let mut registry = registry_with_python();
    registry.providers[1].package_roots = vec![
        ".".to_string(),
        "languages/python-lang-project-harness".to_string(),
    ];

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "python3 - <<'PY'\nfrom pathlib import Path\nprint(Path('languages/python-lang-project-harness/src/python_lang_project_harness/_cli_query_args.py').read_text())\nPY"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(
        decision.subject.paths,
        [
            "languages/python-lang-project-harness/src/python_lang_project_harness/_cli_query_args.py"
        ]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "asp-python");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "search",
            "owner",
            "src/python_lang_project_harness/_cli_query_args.py",
            "items",
            "--query",
            "languages/python-lang-project-harness/src/python_lang_project_harness/_cli_query_args.py",
            "--workspace",
            "languages/python-lang-project-harness",
            "--view",
            "seeds"
        ]
    );
}
