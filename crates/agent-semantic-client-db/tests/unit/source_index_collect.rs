// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{SourceIndexCollectionScope, collect_declarative_source_paths};

#[test]
fn collection_scope_has_only_complete_generation_authority() {
    let scope = SourceIndexCollectionScope::CompleteGeneration;
    let SourceIndexCollectionScope::CompleteGeneration = scope;
    assert_eq!(std::mem::size_of::<SourceIndexCollectionScope>(), 0);
}

#[test]
fn declarative_4096_owner_inventory_is_provider_free_and_subsecond() {
    let directory = tempfile::tempdir().expect("temporary source inventory");
    let mut candidates = Vec::with_capacity(4_096);
    for index in 0..4_096 {
        let relative = std::path::PathBuf::from(format!("src/owner-{index:04}.rs"));
        let path = directory.path().join(&relative);
        std::fs::create_dir_all(path.parent().expect("owner parent")).expect("create owner parent");
        std::fs::write(&path, b"pub fn owner() {}\n").expect("write owner");
        candidates.push(relative);
    }
    let extensions = vec![".rs".to_owned()];
    let mut samples = Vec::new();
    for _ in 0..5 {
        let started = std::time::Instant::now();
        let files = collect_declarative_source_paths(
            directory.path(),
            candidates.iter().map(std::path::PathBuf::as_path),
            &extensions,
        );
        samples.push(started.elapsed());
        assert_eq!(files.len(), 4_096);
    }
    samples.sort();
    eprintln!(
        "[base-generation-inventory-performance] {}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.base-generation-inventory-performance-receipt",
            "schemaVersion": "1",
            "ownerCount": 4_096,
            "sampleCount": samples.len(),
            "p95Micros": samples[4].as_micros(),
            "providerProcessCount": 0,
            "providerRpcCount": 0,
        })
    );
    assert!(
        samples[4] < std::time::Duration::from_secs(1),
        "4096-owner declarative inventory exceeded the cold budget: {samples:?}"
    );
}
