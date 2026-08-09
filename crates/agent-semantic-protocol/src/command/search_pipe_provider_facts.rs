//! Provider query-pack context for parser-owned search intent construction.

use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};

pub(super) struct ProviderGraphFactsContext<'a> {
    pub(super) provider: &'a ActivatedProvider,
    pub(super) profiles: &'a RuntimeProfiles,
}

pub(super) fn with_query_pack_descriptor<R>(
    context: Option<&ProviderGraphFactsContext<'_>>,
    f: impl FnOnce(agent_semantic_search::SearchPipeQueryPackDescriptor<'_>) -> R,
) -> Result<R, String> {
    let provider = context.map(|context| context.provider).ok_or_else(|| {
        "provider query-pack descriptor is required for parser-owned search".to_string()
    })?;
    with_activated_provider_query_pack_descriptor(provider, f)
}

fn provider_query_pack_role(
    role: agent_semantic_hook::ProviderQueryPackTermRole,
) -> agent_semantic_search::SearchPipeTermRole {
    match role {
        agent_semantic_hook::ProviderQueryPackTermRole::Context => {
            agent_semantic_search::SearchPipeTermRole::Context
        }
        agent_semantic_hook::ProviderQueryPackTermRole::Concept => {
            agent_semantic_search::SearchPipeTermRole::Concept
        }
        agent_semantic_hook::ProviderQueryPackTermRole::Symbol => {
            agent_semantic_search::SearchPipeTermRole::Symbol
        }
        agent_semantic_hook::ProviderQueryPackTermRole::Literal => {
            agent_semantic_search::SearchPipeTermRole::Literal
        }
        agent_semantic_hook::ProviderQueryPackTermRole::DiagnosticCode => {
            agent_semantic_search::SearchPipeTermRole::DiagnosticCode
        }
    }
}

fn with_activated_provider_query_pack_descriptor<R>(
    provider: &agent_semantic_hook::ActivatedProvider,
    f: impl FnOnce(agent_semantic_search::SearchPipeQueryPackDescriptor<'_>) -> R,
) -> Result<R, String> {
    let descriptor = &provider.query_pack_descriptor;
    let term_role_overrides = descriptor
        .term_role_overrides()
        .iter()
        .map(
            |override_| agent_semantic_search::SearchPipeQueryPackTermRoleOverride {
                term: &override_.term,
                role: provider_query_pack_role(override_.role),
                case_sensitive: override_.case_sensitive,
            },
        )
        .collect::<Vec<_>>();
    let clause_role_sets = descriptor
        .recipes()
        .iter()
        .map(|recipe| {
            recipe
                .clauses
                .iter()
                .map(|clause| {
                    clause
                        .roles
                        .iter()
                        .copied()
                        .map(provider_query_pack_role)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let clause_sets = descriptor
        .recipes()
        .iter()
        .zip(&clause_role_sets)
        .map(|(recipe, role_sets)| {
            recipe
                .clauses
                .iter()
                .zip(role_sets)
                .map(
                    |(clause, roles)| agent_semantic_search::SearchPipeQueryPackClause {
                        terms: &clause.terms,
                        roles,
                        intent_axes: &clause.intent_axes,
                    },
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let recipes = descriptor
        .recipes()
        .iter()
        .zip(&clause_sets)
        .map(
            |(recipe, clauses)| agent_semantic_search::SearchPipeQueryPackRecipe {
                recipe_id: &recipe.recipe_id,
                trigger_terms: &recipe.trigger.terms,
                trigger_match: &recipe.trigger.r#match,
                clauses,
            },
        )
        .collect::<Vec<_>>();
    Ok(f(agent_semantic_search::SearchPipeQueryPackDescriptor {
        descriptor_id: descriptor.descriptor_id(),
        descriptor_version: descriptor.descriptor_version(),
        language_id: descriptor.language_id(),
        term_role_overrides: &term_role_overrides,
        recipes: &recipes,
    }))
}
