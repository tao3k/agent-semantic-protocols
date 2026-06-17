use agent_semantic_hook::{
    ActivatedProvider, DecisionKind, HookDecision, HookRuntime, ReasonKind, StdinMode,
    classify_hook,
};
use serde_json::json;

use crate::classifier::{command, command_with_stdin, provider, provider_routes};

const HOOK_TRIGGER_PROMPT_TEMPLATE: &str =
    include_str!("../../../../templates/hook_trigger_prompt.md");

pub(super) fn assert_raw_search_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
    for route in &decision.routes {
        assert!(
            !route
                .argv
                .windows(2)
                .any(|window| window[0] == "search" && window[1] == "query"),
            "{command}: {:?}",
            route.argv
        );
        assert!(
            !route.argv.iter().any(|arg| arg == "direct-source-read"),
            "{command}: {:?}",
            route.argv
        );
        if let Some(surface_index) = route.argv.iter().position(|arg| arg == "--surface") {
            assert_eq!(
                route.argv.get(surface_index + 1).map(String::as_str),
                Some("owners,tests"),
                "{command}: {:?}",
                route.argv
            );
        }
    }
    assert!(
        !decision.message.contains("direct-source-read"),
        "{command}: {}",
        decision.message
    );
}

pub(super) fn assert_bulk_source_dump_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
}

pub(super) fn assert_direct_read_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
}

pub(super) fn assert_content_dump_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
}

fn assert_agent_facade_decision(decision: &HookDecision, command: &str) {
    let expected_message = managed_prompt_template()
        .replace("{reason}", reason_kind_label(decision.reason_kind))
        .replace("{routes}", &routes_markdown_for_test(&decision.routes));
    assert_eq!(decision.message, expected_message, "{command}");
    for route in &decision.routes {
        assert_eq!(route.binary, "asp", "{command}: {:?}", route.argv);
        assert_eq!(
            route.argv.first().map(String::as_str),
            Some("asp"),
            "{command}"
        );
    }
    for stale in [
        "rs-harness agent guide",
        "ts-harness agent guide",
        "py-harness agent guide",
        "provider guide:",
        "blocked=",
        "protocol=asp-hook-recovery.v1",
        "|run-next",
        "|guide",
    ] {
        assert!(
            !decision.message.contains(stale),
            "{command}: {}",
            decision.message
        );
    }
}

fn managed_prompt_template() -> &'static str {
    HOOK_TRIGGER_PROMPT_TEMPLATE
        .split("<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-BEGIN -->")
        .nth(1)
        .expect("managed prompt begin")
        .split("<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-END -->")
        .next()
        .expect("managed prompt end")
        .trim_matches('\n')
}

fn routes_markdown_for_test(routes: &[agent_semantic_hook::DecisionRoute]) -> String {
    if routes.is_empty() {
        return "```sh\nasp guide\n```".to_string();
    }
    routes
        .iter()
        .map(|route| format!("```sh\n{}\n```", command_line_for_test(&route.argv)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn command_line_for_test(argv: &[String]) -> String {
    let argv = display_argv_for_test(argv);
    argv.iter()
        .map(|arg| {
            if arg.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '-' | '_' | '.' | '/' | ':')
            }) {
                arg.to_string()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_argv_for_test(argv: &[String]) -> Vec<String> {
    if !uses_agent_facade_workspace_positional_for_test(argv) {
        return argv.to_vec();
    }

    let workspace = argv[argv.len() - 1].clone();
    let mut rendered = argv[..argv.len() - 1].to_vec();
    let insert_at = rendered
        .iter()
        .position(|arg| arg == "--view")
        .unwrap_or(rendered.len());
    rendered.insert(insert_at, "--workspace".to_string());
    rendered.insert(insert_at + 1, workspace);
    rendered
}

fn uses_agent_facade_workspace_positional_for_test(argv: &[String]) -> bool {
    if argv.len() < 4 || argv.iter().any(|arg| arg == "--workspace") {
        return false;
    }
    if !matches!(argv.first().map(String::as_str), Some("asp")) {
        return false;
    }
    if !matches!(argv.get(2).map(String::as_str), Some("query" | "search")) {
        return false;
    }
    argv.last()
        .is_some_and(|arg| !arg.is_empty() && !arg.starts_with('-'))
}

fn reason_kind_label(reason_kind: ReasonKind) -> &'static str {
    match reason_kind {
        ReasonKind::None => "none",
        ReasonKind::DirectSourceRead => "direct-source-read",
        ReasonKind::BulkSourceDump => "bulk-source-dump",
        ReasonKind::RawBroadSearch => "raw-broad-search",
        ReasonKind::SourceDirectoryEnumeration => "source-directory-enumeration",
        ReasonKind::AgentSearchJson => "agent-search-json",
        ReasonKind::SemanticAstPatchRequired => "semantic-ast-patch-required",
        ReasonKind::SubagentReceiptRequired => "subagent-receipt-required",
    }
}

pub(super) fn assert_allowed(command: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
}

pub(super) fn rust_registry() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![rust_provider()],
    }
}

pub(super) fn polyglot_registry() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), rust_provider(), python_provider()],
    }
}

pub(super) fn document_registry() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![markdown_provider()],
    }
}

pub(super) fn markdown_provider() -> ActivatedProvider {
    provider(
        "md",
        "orgize",
        "asp",
        "agent.semantic-protocols.languages.md.orgize",
        &[".md", ".markdown"],
        &[],
        &["."],
        &[".git"],
        provider_routes(
            "asp",
            Some(command(&[
                "asp", "md", "query", "--term", "{query}", "--view", "metadata", ".",
            ])),
        ),
    )
}

pub(super) fn typescript_provider() -> ActivatedProvider {
    provider(
        "typescript",
        "ts-harness",
        "ts-harness",
        "agent.semantic-protocols.languages.typescript.ts-harness",
        &[".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"],
        &["package.json", "tsconfig.json"],
        &["src", "test", "tests", "__tests__", "packages", "apps"],
        &["node_modules", "dist", ".git"],
        provider_routes(
            "ts-harness",
            Some(command(&[
                "asp",
                "typescript",
                "query",
                "--selector",
                "{selector}",
                "{termArgs}",
                "--surface",
                "owners,tests",
                "--workspace",
                ".",
                "--view",
                "seeds",
            ])),
        ),
    )
}

pub(super) fn rust_provider() -> ActivatedProvider {
    let mut routes = provider_routes(
        "rs-harness",
        Some(command(&[
            "asp",
            "rust",
            "query",
            "--selector",
            "{selector}",
            "{termArgs}",
            "--surface",
            "owners,tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])),
    );
    routes.owner = command(&[
        "asp", "rust", "search", "owner", "{path}", "items", "--view", "seeds", ".",
    ]);
    routes.ingest = command_with_stdin(
        &[
            "asp", "rust", "search", "ingest", "items", "tests", "--view", "seeds", ".",
        ],
        StdinMode::PipeCandidates,
    );
    provider(
        "rust",
        "rs-harness",
        "rs-harness",
        "agent.semantic-protocols.languages.rust.rs-harness",
        &[".rs"],
        &["Cargo.toml", "Cargo.lock"],
        &["src", "tests", "crates", "examples", "benches"],
        &["target", ".git"],
        routes,
    )
}

pub(super) fn python_provider() -> ActivatedProvider {
    let mut routes = provider_routes(
        "py-harness",
        Some(command(&[
            "asp",
            "python",
            "query",
            "--selector",
            "{selector}",
            "{termArgs}",
            "--surface",
            "owners,tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])),
    );
    routes.owner = command(&["py-harness", "search", "owner", "{path}", "."]);
    routes.ingest = command_with_stdin(
        &["py-harness", "search", "ingest", "."],
        StdinMode::PipeCandidates,
    );
    provider(
        "python",
        "py-harness",
        "py-harness",
        "agent.semantic-protocols.languages.python.py-harness",
        &[".py", ".pyi"],
        &["pyproject.toml", "setup.py", "setup.cfg"],
        &["src", "test", "tests", "packages"],
        &[".venv", "__pycache__", ".git"],
        routes,
    )
}
