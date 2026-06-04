use std::path::PathBuf;

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, ProviderRegistrySnapshot, ResolvedProvider,
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

fn provider(language_id: &str, binary: &str) -> ResolvedProvider {
    ResolvedProvider {
        language_id: language_id.into(),
        provider_id: binary.into(),
        binary: binary.to_string(),
        provider_command_prefix: vec![
            "direnv".to_string(),
            "exec".to_string(),
            ".".to_string(),
            binary.to_string(),
        ],
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
