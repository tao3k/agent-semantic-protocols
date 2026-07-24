use agent_semantic_search::{
    SearchPipeQueryPackClause, SearchPipeQueryPackDescriptor, SearchPipeQueryPackRecipe,
    SearchPipeQueryPackTermRoleOverride,
};

pub(crate) fn with_typescript_query_pack<R>(
    language_id: &str,
    use_descriptor: impl FnOnce(SearchPipeQueryPackDescriptor<'_>) -> R,
) -> R {
    let role_overrides = [SearchPipeQueryPackTermRoleOverride {
        term: "Effect",
        role: "context",
        case_sensitive: true,
    }];
    let trigger_terms = vec!["Queue".to_string(), "Stream".to_string()];
    let recipe_terms = vec![
        "Queue".to_string(),
        "Stream".to_string(),
        "backpressure".to_string(),
    ];
    let recipe_clauses = [SearchPipeQueryPackClause {
        terms: &recipe_terms,
        roles: &[],
        intent_axes: &[],
    }];
    let recipes = [SearchPipeQueryPackRecipe {
        recipe_id: "typescript.queue-stream-backpressure",
        trigger_terms: &trigger_terms,
        trigger_match: "all",
        clauses: &recipe_clauses,
    }];
    use_descriptor(SearchPipeQueryPackDescriptor {
        descriptor_id: "typescript.query-pack",
        descriptor_version: "1",
        language_id,
        term_role_overrides: &role_overrides,
        recipes: &recipes,
    })
}
