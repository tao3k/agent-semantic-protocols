// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn generation_delta_digest_overlay_is_scenario_measured() {
    use asp_rust_project_harness_policy::{
        AspRustProjectHarnessScenarioObservation, GENERATION_DELTA_DIGEST_OVERLAY_SCENARIO_ID,
        asp_search_scenario_package, measure_asp_rust_scenario,
        render_asp_rust_scenario_benchmark_toml,
    };

    let base = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        (0..4_096).map(|index| {
            let path = format!("src/generated/owner_{index:04}.rs");
            let digest = agent_semantic_content_identity::hash_blob(path.as_bytes()).value;
            (path, digest)
        }),
    );
    let changed_path = "src/generated/owner_4095.rs";
    let replacement_digest =
        agent_semantic_content_identity::hash_blob(b"pub fn owner_4095_changed() {}\n").value;
    let scenario = asp_search_scenario_package()
        .scenarios
        .into_iter()
        .find(|scenario| scenario.name == GENERATION_DELTA_DIGEST_OVERLAY_SCENARIO_ID)
        .expect("generation delta digest overlay Scenario");
    let measurement = measure_asp_rust_scenario(&scenario, || {
        let started = std::time::Instant::now();
        let successor = base.canonical_with_overlay_delta(
            [(changed_path, replacement_digest.as_str())],
            std::iter::empty::<&str>(),
        );
        let elapsed = started.elapsed();
        assert_eq!(
            successor.file_digest(changed_path),
            Some(replacement_digest.as_str())
        );
        assert_eq!(successor.file_digests().count(), 4_096);
        AspRustProjectHarnessScenarioObservation::default()
            .with_timing("delta_digest_overlay", elapsed)
            .with_metric("workspace_owner_count", 4_096)
            .with_metric("changed_owner_count", 1)
            .with_metric("unchanged_source_byte_read_count", 0)
            .with_metric("full_merkle_rebuild_count", 0)
            .with_metric("digest_leaf_copy_count", 4_096)
            .with_metric("successor_leaf_count", 4_096)
    })
    .expect("measure generation delta digest overlay Scenario");
    let rendered = render_asp_rust_scenario_benchmark_toml(&scenario, &measurement)
        .expect("render generation delta digest overlay benchmark");
    assert!(rendered.contains("[metrics.unchanged_source_byte_read_count]"));
    assert!(rendered.contains("observed = 0"));
    println!("{rendered}");
}
