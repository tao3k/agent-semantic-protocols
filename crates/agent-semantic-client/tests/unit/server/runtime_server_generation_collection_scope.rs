use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use agent_semantic_client_db::server_source_index::SourceIndexCollectionScope;

use super::runtime_server_daemon::workspace_generation_collection_scope;

#[test]
fn targeted_query_demand_builds_a_complete_workspace_generation() {
    let scope =
        workspace_generation_collection_scope(Path::new("/workspace"), &BTreeSet::new(), true)
            .expect("targeted query demand scope");

    assert_eq!(scope, SourceIndexCollectionScope::CompleteGeneration);
}

#[test]
fn changed_owner_mutation_remains_an_explicit_owner_scope() {
    let changed_paths = BTreeSet::from([
        PathBuf::from("/workspace/src/a.rs"),
        PathBuf::from("/workspace/src/b.rs"),
    ]);
    let scope =
        workspace_generation_collection_scope(Path::new("/workspace"), &changed_paths, false)
            .expect("changed owner scope");

    assert_eq!(
        scope,
        SourceIndexCollectionScope::ExplicitOwners {
            owner_paths: vec!["src/a.rs".to_owned(), "src/b.rs".to_owned()],
        }
    );
}
