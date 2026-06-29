use std::hint::black_box;
use std::path::PathBuf;

use agent_semantic_client_core::{ClientMethod, ClientRequest, ProviderRegistrySnapshot};
use agent_semantic_client_local_cli::LocalNativeCliBackend;
use criterion::{Criterion, criterion_group, criterion_main};

fn backend_hot_path(c: &mut Criterion) {
    let root = PathBuf::from(".");
    let backend = LocalNativeCliBackend::new(ProviderRegistrySnapshot {
        activation_path: root.join(".cache/agent-semantic-protocol/hooks/activation.json"),
        providers: Vec::new(),
    });
    let request = ClientRequest::new(ClientMethod::Search, root)
        .with_language("rust")
        .with_forwarded_args(vec![
            "lexical".to_string(),
            "cache replay".to_string(),
            "--view=seeds".to_string(),
            ".".to_string(),
        ]);
    c.bench_function("backend_hot_path", |b| {
        b.iter(|| {
            let result = backend.prepare(black_box(&request));
            black_box(result.is_err());
        });
    });
}

criterion_group!(benches, backend_hot_path);
criterion_main!(benches);
