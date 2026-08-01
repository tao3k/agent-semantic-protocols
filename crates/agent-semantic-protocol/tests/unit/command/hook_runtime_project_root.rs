use super::{hook_runtime_project_root_override, with_hook_runtime_project_root_override};
use std::path::PathBuf;

#[test]
fn resident_hook_project_root_override_is_scoped_and_nested() {
    assert_eq!(hook_runtime_project_root_override(), None);

    let outer = PathBuf::from("/workspace/outer");
    let inner = PathBuf::from("/workspace/inner");
    with_hook_runtime_project_root_override(outer.clone(), || {
        assert_eq!(hook_runtime_project_root_override(), Some(outer.clone()));
        with_hook_runtime_project_root_override(inner.clone(), || {
            assert_eq!(hook_runtime_project_root_override(), Some(inner));
        });
        assert_eq!(hook_runtime_project_root_override(), Some(outer));
    });

    assert_eq!(hook_runtime_project_root_override(), None);
}

#[test]
fn resident_hook_project_root_override_is_restored_after_panic() {
    let result = std::panic::catch_unwind(|| {
        with_hook_runtime_project_root_override(PathBuf::from("/workspace/panic"), || {
            panic!("exercise override guard");
        });
    });

    assert!(result.is_err());
    assert_eq!(hook_runtime_project_root_override(), None);
}
