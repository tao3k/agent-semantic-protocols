#[path = "../../src/exact_projection_diagnostic.rs"]
mod implementation;
use implementation::{ProviderExactResolutionFacts, resolution_from_facts, validate_resolution};

#[test]
fn resident_exact_adapter_does_not_own_diagnostic_policy() {
    let adapter_source = include_str!("../../src/command/provider_resident_exact.rs");
    let command_modules = include_str!("../../src/command/mod.rs");
    let adapter = include_str!("../../src/exact_projection_diagnostic_io.rs");
    for forbidden in [
        "reasonKind=",
        "owner-not-in-workspace",
        "render_human_diagnostic",
        "ProviderNativeExactResolution {",
        "ProviderNativeExactRecommendedNext {",
        "ASP_EXACT_QUERY_TRACE",
        "fn trace(",
        "fn trace_memory",
        "--code",
        "--names-only",
        "legacy",
        "tokio::fs::",
        "owner_content_digest",
        "runtime_server_workspace_session_for_admission_async",
        "refresh_if_changed",
    ] {
        assert!(
            !adapter_source.contains(forbidden),
            "resident exact adapter owns diagnostic policy token: {forbidden}"
        );
    }
    assert!(adapter_source.contains("crate::exact_projection_diagnostic_io::"));
    assert!(adapter_source.contains("runtime_server_workspace_exact_projection_async"));
    assert!(!adapter_source.contains("ensure_runtime_generation_ready"));
    assert!(
        !adapter_source.contains("runtime_server_workspace_session_async"),
        "exact read hot path must use the mmap generation and never a workspace IPC session"
    );
    assert!(!command_modules.contains("provider_exact_diagnostic_io"));
    assert!(!adapter.contains("command::"));
    assert!(!adapter.contains("provider_direct_exact"));
}

#[test]
fn provider_native_owner_search_is_independent_from_pipe_and_workspace_generation() {
    let adapter_source = include_str!("../../src/command/search_owner_items.rs");
    assert!(adapter_source.contains("project_provider_owner"));
    assert!(adapter_source.contains("runtime_server_stateless_search_session_async"));
    for forbidden in [
        "runtime_server_workspace_session_async",
        "runtime_server_workspace_session_for_admission_async",
        "ensure_runtime_owner(",
        "UnixStream",
        "ensure_runtime_generation_ready",
        "search_pipe",
    ] {
        assert!(
            !adapter_source.contains(forbidden),
            "owner search hot path contains forbidden I/O token: {forbidden}"
        );
    }
    assert!(adapter_source.contains("providerInvocations=1"));
    assert!(adapter_source.contains("controlRoundtrips=0"));
}

#[test]
fn owner_command_dispatch_is_not_part_of_the_search_pipe_engine() {
    let dispatch = include_str!("../../src/command/provider_dispatch.rs");
    let modules = include_str!("../../src/command/mod.rs");

    assert!(dispatch.contains("is_search_owner_items_query(&command_args)"));
    assert!(dispatch.contains("run_search_owner_items_query_command("));
    assert!(modules.contains("mod search_owner_items;"));
    assert!(!modules.contains("mod search_pipe;"));
    assert!(!dispatch.contains("run_asp_fast_search_command"));
}

#[test]
fn production_exact_query_surface_has_no_removed_projection_flags() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![source_root];
    let mut violations = Vec::new();
    let removed = [concat!("--", "code"), concat!("--", "names-only")];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("read production source directory") {
            let path = entry.expect("read production source entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read production Rust source");
            for token in removed {
                if contains_command_token(&source, token) {
                    violations.push(format!("{} contains {token}", path.display()));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "removed exact projection flags remain in production:\n{}",
        violations.join("\n")
    );
}

fn contains_command_token(source: &str, token: &str) -> bool {
    source.match_indices(token).any(|(index, _)| {
        source[index + token.len()..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_ascii_alphanumeric() && next != '-' && next != '_')
    })
}

use implementation::{ProviderNativeExactResolution, render_provider_exact_resolution};

fn missing_resolution() -> ProviderNativeExactResolution {
    ProviderNativeExactResolution {
        schema_id: "agent.semantic-protocols.provider-native-exact-projection".to_owned(),
        schema_version: "1".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "rs-harness".to_owned(),
        owner_path: "src/runtime_server.rs".to_owned(),
        requested_structural_selector: "rust://src/runtime_server.rs#item/function/missing"
            .to_owned(),
        resolution_state: "item-missing".to_owned(),
        reason_kind: "item-not-in-live-owner".to_owned(),
        active_generation_digest:
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        root_digest: "b".repeat(64),
        item_kind: "function".to_owned(),
        item_name: "missing".to_owned(),
        candidates: vec!["rust://src/runtime_server.rs#item/function/live".to_owned()],
        actual_kinds: Vec::new(),
    }
}

#[test]
fn default_resolution_is_a_human_failure_not_json_stdout() {
    let diagnostic = render_provider_exact_resolution(&missing_resolution());
    assert!(!diagnostic.starts_with('{'));
    assert!(diagnostic.contains("state=item-missing"));
    assert!(diagnostic.contains("reasonKind=item-not-in-live-owner"));
    assert!(!diagnostic.contains(" next="));
}

#[test]
fn diagnostic_owner_constructs_stale_selector_receipt() {
    let resolution = resolution_from_facts(ProviderExactResolutionFacts {
        language_id: "rust".to_owned(),
        provider_id: "rs-harness".to_owned(),
        owner_path: "src/runtime_server.rs".to_owned(),
        structural_selector: "rust://src/runtime_server.rs#item/function/missing".to_owned(),
        resolution_state: "selector-stale".to_owned(),
        reason_kind: "selector-not-in-active-generation".to_owned(),
        active_generation_digest:
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        root_digest: "root-current".to_owned(),
        item_kind: "function".to_owned(),
        item_name: "missing".to_owned(),
        candidates: Vec::new(),
        actual_kinds: Vec::new(),
    });

    assert_eq!(resolution.resolution_state, "selector-stale");
    assert_eq!(resolution.reason_kind, "selector-not-in-active-generation");
    assert_eq!(
        resolution.active_generation_digest,
        "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(resolution.item_kind, "function");
    assert_eq!(resolution.item_name, "missing");
}

#[test]
fn active_owner_item_missing_is_terminal_and_cannot_repeat_discovery() {
    let resolution = resolution_from_facts(ProviderExactResolutionFacts {
        language_id: "rust".to_owned(),
        provider_id: "rs-harness".to_owned(),
        owner_path: "src/runtime_server.rs".to_owned(),
        structural_selector: "rust://src/runtime_server.rs#item/function/definitely_missing"
            .to_owned(),
        resolution_state: "item-missing".to_owned(),
        reason_kind: "item-not-in-live-owner".to_owned(),
        active_generation_digest:
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        root_digest: "b".repeat(64),
        item_kind: "function".to_owned(),
        item_name: "definitely_missing".to_owned(),
        candidates: vec!["rust://src/runtime_server.rs#item/function/live".to_owned()],
        actual_kinds: Vec::new(),
    });

    let diagnostic = render_provider_exact_resolution(&resolution);
    assert!(diagnostic.contains("state=item-missing"));
    assert!(!diagnostic.contains(" next="));
}

#[test]
fn diagnostic_owner_rejects_resolution_identity_drift() {
    let mut resolution = missing_resolution();
    resolution.owner_path = "src/other.rs".to_owned();

    let error = validate_resolution(
        &resolution,
        "rust",
        "rs-harness",
        "src/runtime_server.rs",
        "rust://src/runtime_server.rs#item/function/missing",
    )
    .expect_err("diagnostic owner must reject a receipt for another owner");

    assert!(error.contains("ownerPath mismatch"));
}
