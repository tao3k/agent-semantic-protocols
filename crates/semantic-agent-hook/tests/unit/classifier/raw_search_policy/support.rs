use semantic_agent_hook::{
    ActivatedProvider, DecisionKind, HookRuntime, ReasonKind, StdinMode, classify_hook,
};
use serde_json::json;

use crate::classifier::{command, command_with_stdin, provider, provider_routes};

pub(super) fn assert_raw_search_denied(command: &str, binary: &str) {
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
    assert_eq!(decision.routes[0].binary, binary, "{command}");
}

pub(super) fn assert_bulk_source_dump_denied(command: &str, binary: &str) {
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
    assert_eq!(decision.routes[0].binary, binary, "{command}");
}

pub(super) fn assert_direct_read_denied(command: &str, binary: &str) {
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
    assert_eq!(decision.routes[0].binary, binary, "{command}");
}

pub(super) fn assert_content_dump_denied(command: &str, binary: &str) {
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
    assert_eq!(decision.routes[0].binary, binary, "{command}");
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
                "ts-harness",
                "search",
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "{selector}",
                "{termArgs}",
                "--surface",
                "owner,tests",
                "--view",
                "seeds",
                ".",
            ])),
        ),
    )
}

pub(super) fn rust_provider() -> ActivatedProvider {
    let mut routes = provider_routes("rs-harness", None);
    routes.owner = command(&[
        "rs-harness",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "{path}",
        ".",
    ]);
    routes.ingest = command_with_stdin(
        &[
            "rs-harness",
            "search",
            "ingest",
            "items",
            "tests",
            "--view",
            "seeds",
            ".",
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
    let mut routes = provider_routes("py-harness", None);
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
