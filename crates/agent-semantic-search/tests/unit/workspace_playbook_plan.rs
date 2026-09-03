use crate::{
    SearchPlaybookRequest, WorkspaceSearchPlanBinding, WorkspaceSearchProvider,
    WorkspaceSearchSkipReason, WorkspaceSearchStageKind, WorkspaceSearchStagePolicy,
    build_workspace_search_playbook_plan,
};

fn request(intent: &str) -> SearchPlaybookRequest {
    SearchPlaybookRequest {
        query: "Runtime owner Graph".to_owned(),
        intent: intent.to_owned(),
        scope: "workspace".to_owned(),
        coverage: "candidates".to_owned(),
        max_owners: 32,
        deadline_ms: 700,
        explain: "compact".to_owned(),
        language: None,
        workspace: ".".to_owned(),
    }
}

fn binding() -> WorkspaceSearchPlanBinding {
    WorkspaceSearchPlanBinding {
        project_id: "project-1".to_owned(),
        workspace_id: "workspace-1".to_owned(),
        content_generation_digest: format!("blake3-256:{}", "a".repeat(64)),
    }
}

fn provider(language: &str, search_supported: bool) -> WorkspaceSearchProvider {
    WorkspaceSearchProvider {
        language_id: language.to_owned(),
        provider_id: format!("asp-{language}"),
        source_extensions: vec![format!(".{language}")],
        search_supported,
    }
}

#[test]
fn workspace_plan_fans_out_deterministically_over_admitted_languages() {
    let plan = build_workspace_search_playbook_plan(
        &request("relationship"),
        binding(),
        [provider("rust", true), provider("python", true)],
    )
    .unwrap();
    assert_eq!(
        plan.routes
            .iter()
            .map(|route| route.language_id.as_str())
            .collect::<Vec<_>>(),
        ["python", "rust"]
    );
    assert_eq!(plan.normalized_terms, ["graph", "owner", "runtime"]);
    assert!(plan.routes.iter().all(|route| {
        route.generation_digest == plan.content_generation_digest
            && route.stages[0].stage == WorkspaceSearchStageKind::RgAcquisition
            && route.stages[1].stage == WorkspaceSearchStageKind::ProviderNativeSyntax
            && route.stages[2].stage == WorkspaceSearchStageKind::TantivyLexical
            && route.stages[2].policy == WorkspaceSearchStagePolicy::AcceleratorIfReady
            && route.stages[3].policy == WorkspaceSearchStagePolicy::IntentRequired
    }));
    assert_eq!(plan.work.filesystem_read_count, 0);
    assert_eq!(plan.work.provider_process_count, 0);
}

#[test]
fn language_constraint_is_a_typed_plan_filter_not_a_second_command_surface() {
    let mut request = request("conceptual");
    request.language = Some("rust".to_owned());
    let plan = build_workspace_search_playbook_plan(
        &request,
        binding(),
        [provider("rust", true), provider("python", true)],
    )
    .unwrap();
    assert_eq!(plan.routes.len(), 1);
    assert_eq!(plan.routes[0].language_id, "rust");
    assert_eq!(
        plan.skipped_languages[0].reason_kind,
        WorkspaceSearchSkipReason::LanguageNotSelected
    );
    assert_eq!(
        plan.routes[0].stages[3].policy,
        WorkspaceSearchStagePolicy::Skipped
    );
}

#[test]
fn provider_without_search_route_is_explicitly_skipped() {
    let plan = build_workspace_search_playbook_plan(
        &request("conceptual"),
        binding(),
        [provider("rust", false)],
    )
    .unwrap();
    assert!(plan.routes.is_empty());
    assert_eq!(
        plan.skipped_languages[0].reason_kind,
        WorkspaceSearchSkipReason::IntentNotSupported
    );
}

#[test]
fn duplicate_language_authority_fails_closed() {
    let error = build_workspace_search_playbook_plan(
        &request("conceptual"),
        binding(),
        [provider("rust", true), provider("rust", true)],
    )
    .unwrap_err();
    assert_eq!(
        error,
        "workspace playbook requires one provider authority per language"
    );
}
