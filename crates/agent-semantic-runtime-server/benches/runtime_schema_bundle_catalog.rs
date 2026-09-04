use agent_semantic_runtime_server::RuntimeSchemaBundleCatalog;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;

fn runtime_schema_bundle_catalog(c: &mut Criterion) {
    c.bench_function("embedded_catalog_decode", |bench| {
        bench.iter(|| RuntimeSchemaBundleCatalog::load_embedded().expect("embedded catalog"));
    });

    let catalog = RuntimeSchemaBundleCatalog::load_embedded().expect("embedded catalog");
    c.bench_function("workspace_binding_digest", |bench| {
        bench.iter(|| {
            catalog
                .binding_digest(["rust"])
                .expect("registered Rust bundle")
        });
    });
}

criterion_group!(benches, runtime_schema_bundle_catalog);
criterion_main!(benches);
