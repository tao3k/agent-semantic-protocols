use agent_semantic_client_core::{ClientMethod, ClientRequest};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// Measures only the process-local request value construction owned by the
/// client crate. Search execution itself is Runtime/Tokio-owned and therefore
/// has no direct client-directory DB benchmark in this binary.
fn client_request_hot_path(c: &mut Criterion) {
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "lexical".to_string(),
        "cache replay".to_string(),
        "--view=seeds".to_string(),
        ".".to_string(),
    ]);
    c.bench_function("client_request_hot_path", |b| {
        b.iter(|| {
            black_box(request.forwarded_args.len());
            black_box(&request);
        });
    });
}

criterion_group!(benches, client_request_hot_path);
criterion_main!(benches);
