use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    ASP_SYNTAX_QUERY_CAPTURES_ARG, ASP_SYNTAX_QUERY_FIELDS_ARG, ASP_SYNTAX_QUERY_NODE_TYPES_ARG,
    ByteCount, ClientMethod, ClientRequest, ProviderExecution, ProviderRegistrySnapshot,
    ResolvedProvider, RuntimeProfileStatus,
};
use agent_semantic_client_local_cli::LocalNativeCliBackend;

#[test]
fn prepares_registry_owned_provider_command() {
    let backend = LocalNativeCliBackend::new(snapshot(vec![provider("rust", "rs-harness")]));
    let request = ClientRequest::new(ClientMethod::Search, PathBuf::from("/repo"))
        .with_language("rust")
        .with_forwarded_args(vec![
            "owner".to_string(),
            "src/lib.rs".to_string(),
            ".".to_string(),
        ]);

    let command = backend.prepare(&request).expect("prepare command");

    assert_eq!(command.program, "direnv");
    assert_eq!(
        command.args,
        vec![
            "exec",
            ".",
            "rs-harness",
            "search",
            "owner",
            "src/lib.rs",
            "."
        ]
    );
    assert_eq!(command.provider.language_id, "rust");
}

#[test]
fn requires_language_for_multi_provider_route() {
    let backend = LocalNativeCliBackend::new(snapshot(vec![
        provider("rust", "rs-harness"),
        provider("python", "py-harness"),
    ]));
    let request = ClientRequest::new(ClientMethod::Search, PathBuf::from("/repo"));

    let error = backend.prepare(&request).expect_err("requires language");

    assert!(error.contains("use --language <id>"));
}

#[test]
fn prepares_query_with_asp_compiled_syntax_plan() {
    let backend = LocalNativeCliBackend::new(snapshot(vec![provider("rust", "rs-harness")]));
    let request = ClientRequest::new(ClientMethod::Query, PathBuf::from("/repo"))
        .with_language("rust")
        .with_forwarded_args(vec![
            "--treesitter-query".to_string(),
            "(function_item name: (identifier) @function.name)".to_string(),
            ".".to_string(),
        ]);

    let command = backend.prepare(&request).expect("prepare command");

    assert_eq!(
        command.args,
        vec![
            "exec",
            ".",
            "rs-harness",
            "query",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            ".",
            ASP_SYNTAX_QUERY_CAPTURES_ARG,
            "function.name",
            ASP_SYNTAX_QUERY_NODE_TYPES_ARG,
            "function_item,identifier",
            ASP_SYNTAX_QUERY_FIELDS_ARG,
            "name",
        ]
    );
}

#[test]
fn prepares_catalog_query_with_asp_compiled_syntax_plan() {
    let backend = LocalNativeCliBackend::new(snapshot(vec![provider("rust", "rs-harness")]));
    let request = ClientRequest::new(ClientMethod::Query, PathBuf::from("/repo"))
        .with_language("rust")
        .with_forwarded_args(vec![
            "--catalog".to_string(),
            "declarations".to_string(),
            ".".to_string(),
        ]);

    let command = backend.prepare(&request).expect("prepare command");

    assert!(command.args.windows(2).any(|window| window
        == [
            ASP_SYNTAX_QUERY_CAPTURES_ARG,
            "constant.definition,constant.name,constant.type,function.definition,function.modifier,function.name,function.return_type,function.type_parameters,impl.definition,impl.target,impl.trait,impl.type_parameters,item.attribute,item.visibility,module.definition,module.name,trait.bounds,trait.definition,trait.name,trait.type_parameters,type.aliased_type,type.definition,type.name,type.type_parameters"
        ]));
    assert_internal_arg_contains(
        &command.args,
        ASP_SYNTAX_QUERY_NODE_TYPES_ARG,
        "function_item",
    );
    assert_internal_arg_contains(&command.args, ASP_SYNTAX_QUERY_NODE_TYPES_ARG, "trait_item");
    assert_internal_arg_contains(&command.args, ASP_SYNTAX_QUERY_FIELDS_ARG, "name");
}

#[test]
fn reports_unusable_runtime_profile_status_before_falling_back_to_path() {
    let mut provider = provider("rust", "rs-harness");
    provider.provider_command_prefix.clear();
    provider.runtime_profile_status = Some(RuntimeProfileStatus::Missing);
    let backend = LocalNativeCliBackend::new(snapshot(vec![provider]));
    let request = ClientRequest::new(ClientMethod::Search, PathBuf::from("/repo"))
        .with_language("rust")
        .with_forwarded_args(vec!["prime".to_string(), ".".to_string()]);

    let error = backend
        .prepare(&request)
        .expect_err("missing runtime profile");

    assert!(error.contains("provider `rs-harness` language `rust` is missing"));
    assert!(error.contains("asp hook doctor --client codex ."));
}

#[test]
fn activation_prefix_takes_precedence_over_runtime_profile_argv() {
    let mut provider = provider("rust", "rs-harness");
    provider.runtime_command_argv = Some(vec!["/opt/homebrew/bin/rs-harness".to_string()]);
    provider.runtime_profile_status = Some(RuntimeProfileStatus::Available);
    let backend = LocalNativeCliBackend::new(snapshot(vec![provider]));
    let request = ClientRequest::new(ClientMethod::Search, PathBuf::from("/repo"))
        .with_language("rust")
        .with_forwarded_args(vec![
            "workspace".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
        ]);

    let command = backend.prepare(&request).expect("prepare command");

    assert_eq!(command.program, "direnv");
    assert_eq!(
        command.args,
        vec![
            "exec",
            ".",
            "rs-harness",
            "search",
            "workspace",
            "--view",
            "seeds"
        ]
    );
}

#[test]
fn relative_project_root_is_canonicalized_for_provider_cwd() {
    let backend = LocalNativeCliBackend::new(snapshot(vec![provider("rust", "rs-harness")]));
    let current_dir = std::env::current_dir().expect("current dir");
    let request = ClientRequest::new(ClientMethod::Search, PathBuf::from("."))
        .with_language("rust")
        .with_forwarded_args(vec!["prime".to_string()]);

    let command = backend.prepare(&request).expect("prepare command");

    assert!(command.project_root.is_absolute());
    assert_eq!(command.project_root, current_dir);
}

#[test]
fn execute_records_transport_receipt_fields() {
    let mut provider = provider("rust", "fake-rust-provider");
    provider.provider_command_prefix = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf 'provider-out'; printf 'provider-err' >&2".to_string(),
    ];
    let root = temp_project_root("local-native-receipt");
    let backend = LocalNativeCliBackend::new(snapshot(vec![provider]));
    let request = ClientRequest::new(ClientMethod::Search, root.clone())
        .with_language("rust")
        .with_forwarded_args(vec!["prime".to_string()]);

    let output = backend.execute(&request).expect("execute provider");

    assert_eq!(output.status_code, 0);
    assert_eq!(output.stdout.as_ref(), b"provider-out");
    assert_eq!(output.stderr.as_ref(), b"provider-err");
    assert_eq!(output.receipt.provider_command_count, 1);
    assert_eq!(output.receipt.provider_processes_spawned, 1);
    assert_eq!(
        output.receipt.stdout_bytes,
        ByteCount::from_len(output.stdout.len())
    );
    assert_eq!(
        output.receipt.stderr_bytes,
        ByteCount::from_len(output.stderr.len())
    );
    let command = &output.receipt.provider_commands[0];
    assert_eq!(command.exit_code, 0);
    assert_eq!(
        command.stdout_bytes,
        ByteCount::from_len("provider-out".len())
    );
    assert_eq!(
        command.stderr_bytes,
        ByteCount::from_len("provider-err".len())
    );
    assert!(
        command
            .stdout_sha256
            .as_deref()
            .is_some_and(|hash| hash.len() == 64)
    );
    assert!(
        command
            .stderr_sha256
            .as_deref()
            .is_some_and(|hash| hash.len() == 64)
    );
    assert!(!command.stdout_truncated);
    assert!(!command.stderr_truncated);
    assert!(!command.timed_out);
    assert!(command.elapsed_ms.as_u64() <= output.receipt.elapsed_ms.as_u64());

    let _ = std::fs::remove_dir_all(root);
}

fn provider(language_id: &str, binary: &str) -> ResolvedProvider {
    ResolvedProvider {
        language_id: language_id.into(),
        provider_id: binary.into(),
        binary: binary.to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: vec![
            "direnv".to_string(),
            "exec".to_string(),
            ".".to_string(),
            binary.to_string(),
        ],
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
    }
}

fn snapshot(providers: Vec<ResolvedProvider>) -> ProviderRegistrySnapshot {
    ProviderRegistrySnapshot {
        activation_path: PathBuf::from(
            "/repo/.cache/agent-semantic-protocol/hooks/activation.json",
        ),
        providers,
    }
}

fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-local-cli-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}

fn assert_internal_arg_contains(args: &[String], key: &str, expected_value: &str) {
    let value = args
        .windows(2)
        .find(|window| window[0] == key)
        .map(|window| window[1].as_str())
        .unwrap_or_else(|| panic!("missing internal arg {key}"));
    assert!(
        value.split(',').any(|value| value == expected_value),
        "{key} did not contain {expected_value}: {value}"
    );
}
