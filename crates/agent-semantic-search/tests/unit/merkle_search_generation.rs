use std::sync::Arc;

#[path = "../integration/source_snapshot_evidence_fixture.rs"]
mod source_snapshot_fixture;

use agent_semantic_search::{
    MerkleSearchGeneration, SearchOwnerChange, SearchOwnerFragment, SearchProjectionIdentity,
};

fn identity(root: &str, provider_digest: &str) -> SearchProjectionIdentity {
    SearchProjectionIdentity {
        workspace_identity: "workspace-1".to_owned(),
        source_root_digest: root.to_owned(),
        provider_digest: provider_digest.to_owned(),
        schema_digest: "schema-digest".to_owned(),
        analyzer_digest: "python-and-lexical-analyzer-digest".to_owned(),
    }
}

fn fragment(path: &str, digest: &str, key: &str, graph: &str) -> SearchOwnerFragment {
    SearchOwnerFragment::new(
        path,
        digest,
        10,
        vec![key.to_owned()],
        Some(graph.to_owned()),
    )
    .expect("valid owner fragment")
}

#[test]
fn merkle_change_set_reuses_unchanged_fragments_and_tombstones_deletions() {
    let fixture = source_snapshot_fixture::canonical_test_snapshot();
    let base = MerkleSearchGeneration::apply_change_set(
        None,
        identity("base-root", fixture.evidence.provider_digest.as_str()),
        vec![
            SearchOwnerChange::Added {
                fragment: fragment("src/a.py", "a-1", "old_key", "graph-a-1"),
            },
            SearchOwnerChange::Added {
                fragment: fragment("src/b.py", "b-1", "removed_key", "graph-b-1"),
            },
            SearchOwnerChange::Added {
                fragment: fragment("src/c.py", "c-1", "stable_key", "graph-c-1"),
            },
        ],
    )
    .expect("base generation");
    let stable = Arc::clone(base.owner("src/c.py").expect("stable owner"));

    let candidate = MerkleSearchGeneration::apply_change_set(
        Some(&base),
        identity(
            &fixture.evidence.root_digest,
            fixture.evidence.provider_digest.as_str(),
        ),
        vec![
            SearchOwnerChange::Changed {
                previous_content_digest: "a-1".to_owned(),
                fragment: fragment("src/a.py", "a-2", "changed_key", "graph-a-2"),
            },
            SearchOwnerChange::Removed {
                owner_path: "src/b.py".to_owned(),
                previous_content_digest: "b-1".to_owned(),
            },
        ],
    )
    .expect("candidate generation");

    assert!(Arc::ptr_eq(
        &stable,
        candidate.owner("src/c.py").expect("reused stable owner")
    ));
    assert!(candidate.owner("src/b.py").is_none());
    assert!(candidate.tombstones().contains("src/b.py"));
    assert_eq!(
        candidate
            .owner("src/a.py")
            .and_then(|owner| owner.graph_fragment_digest.as_deref()),
        Some("graph-a-2")
    );

    let resident = candidate
        .resident_source_index(
            fixture.evidence.clone(),
            agent_semantic_config::LanguageId::new("python"),
            agent_semantic_config::ProviderId::new("asp-python"),
            crate::ResidentIndexBuildResources::new(
                1,
                32 * 1024 * 1024,
                crate::ResidentIndexBuildStrategy::SingleSegmentBulk,
            )
            .unwrap(),
        )
        .expect("resident index from the exact candidate manifest");
    assert_eq!(
        resident
            .query("removed_key", None, 8)
            .expect("removed query")
            .hits
            .len(),
        0
    );
    assert_eq!(
        resident
            .query("changed_key", None, 8)
            .expect("changed query")
            .hits[0]
            .owner_path,
        "src/a.py"
    );
}

#[test]
fn merkle_change_set_rejects_unsorted_or_cross_authority_updates() {
    let provider = "provider-digest";
    let unsorted = MerkleSearchGeneration::apply_change_set(
        None,
        identity("root-a", provider),
        vec![
            SearchOwnerChange::Added {
                fragment: fragment("src/z.py", "z", "z", "graph-z"),
            },
            SearchOwnerChange::Added {
                fragment: fragment("src/a.py", "a", "a", "graph-a"),
            },
        ],
    )
    .expect_err("unsorted changes must fail closed");
    assert!(unsorted.contains("path-sorted"));

    let base =
        MerkleSearchGeneration::apply_change_set(None, identity("root-a", provider), Vec::new())
            .expect("empty base generation");
    let cross_authority = MerkleSearchGeneration::apply_change_set(
        Some(&base),
        identity("root-b", "different-provider"),
        Vec::new(),
    )
    .expect_err("provider drift must fail closed");
    assert!(cross_authority.contains("authority changed"));
}
