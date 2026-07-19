use asp_rust_project_harness_policy::{asp_search_scenario_package, asp_workspace_member_policies};
use criterion::{Criterion, criterion_group, criterion_main};

fn policy_lookup_smoke_benchmark(criterion: &mut Criterion) {
    criterion.bench_function("asp_workspace_member_policies", |bencher| {
        bencher.iter(|| asp_workspace_member_policies())
    });
}

fn scenario_package_smoke_benchmark(criterion: &mut Criterion) {
    criterion.bench_function("asp_search_scenario_package", |bencher| {
        bencher.iter(asp_search_scenario_package)
    });
}

criterion_group!(
    performance_verification,
    policy_lookup_smoke_benchmark,
    scenario_package_smoke_benchmark
);
criterion_main!(performance_verification);
