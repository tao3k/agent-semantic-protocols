use crate::workspace_db_ipc::WorkspaceDbIpcResult;

use super::admit_hook_evaluation_result;

fn result(workspace_identity: &str, project_root: &str) -> WorkspaceDbIpcResult {
    WorkspaceDbIpcResult::HookEvaluation {
        workspace_identity: workspace_identity.to_owned(),
        project_root: project_root.to_owned(),
        output: "decision".to_owned(),
    }
}

#[test]
fn hook_evaluation_admits_only_the_session_workspace_and_project() {
    assert_eq!(
        admit_hook_evaluation_result(
            "workspace-current",
            "/project/current",
            result("workspace-current", "/project/current"),
        ),
        Ok("decision".to_owned())
    );

    for rejected in [
        result("workspace-stale", "/project/current"),
        result("workspace-current", "/project/stale"),
    ] {
        let error = admit_hook_evaluation_result("workspace-current", "/project/current", rejected)
            .expect_err("stale resident identity must fail closed");
        assert!(error.contains("identity mismatch"), "{error}");
    }
}
