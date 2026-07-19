//! Provider semantic facts and query-pack descriptor validation.

use crate::protocol::AgentHookError;

use super::protocol_activation_manifest::ProviderManifest;

use std::collections::BTreeSet;

/// Validate the provider-owned query-pack descriptor contract.
pub(super) fn validate_query_pack_descriptor(
    manifest: &ProviderManifest,
) -> Result<(), AgentHookError> {
    let invalid = |reason: &str| {
        AgentHookError::InvalidActivationConfig(format!(
            "provider manifest {} has an invalid queryPackDescriptor: {reason}",
            manifest.manifest_id
        ))
    };
    let descriptor = manifest
        .query_pack_descriptor
        .as_ref()
        .ok_or_else(|| invalid("descriptor is required for every active provider"))?;
    if descriptor.descriptor_id.trim().is_empty()
        || descriptor.descriptor_version != "1"
        || descriptor.language_id != manifest.language_id
        || descriptor.recipes.is_empty()
    {
        return Err(invalid("identity, version, language, or recipes"));
    }
    if let Some(semantic_descriptor_id) = descriptor.semantic_facts_descriptor_id.as_deref()
        && (semantic_descriptor_id.trim().is_empty()
            || manifest
                .semantic_facts_descriptor
                .as_ref()
                .is_none_or(|semantic| semantic.descriptor_id != semantic_descriptor_id))
    {
        return Err(invalid(
            "semanticFactsDescriptorId does not match the provider",
        ));
    }
    let allowed_roles = ["context", "concept", "symbol"];
    for role_override in &descriptor.term_role_overrides {
        if role_override.term.trim().is_empty()
            || !allowed_roles.contains(&role_override.role.as_str())
        {
            return Err(invalid("term role override"));
        }
    }
    let allowed_axes = [
        "data-shape",
        "collection",
        "concurrency",
        "cancellation",
        "resource-lifecycle",
        "stream",
    ];
    let mut recipe_ids = BTreeSet::new();
    for recipe in &descriptor.recipes {
        if recipe.recipe_id.trim().is_empty()
            || !recipe_ids.insert(recipe.recipe_id.as_str())
            || recipe.trigger.terms.is_empty()
            || !matches!(recipe.trigger.r#match.as_str(), "any" | "all")
            || recipe.clauses.is_empty()
            || recipe
                .trigger
                .terms
                .iter()
                .any(|term| term.trim().is_empty())
        {
            return Err(invalid("recipe identity, trigger, or clauses"));
        }
        for clause in &recipe.clauses {
            if clause.terms.is_empty()
                || clause.terms.iter().any(|term| term.trim().is_empty())
                || clause
                    .roles
                    .iter()
                    .any(|role| !allowed_roles.contains(&role.as_str()))
                || clause
                    .intent_axes
                    .iter()
                    .any(|axis| !allowed_axes.contains(&axis.as_str()))
            {
                return Err(invalid("clause terms, roles, or intent axes"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_semantic_facts_descriptor(
    manifest: &ProviderManifest,
) -> Result<(), AgentHookError> {
    let Some(descriptor) = manifest.semantic_facts_descriptor.as_ref() else {
        return Ok(());
    };
    if !manifest.search_capabilities.semantic_facts {
        return Err(AgentHookError::InvalidActivationConfig(format!(
            "provider manifest {} declares semanticFactsDescriptor while semanticFacts is disabled",
            manifest.manifest_id
        )));
    }
    if descriptor.descriptor_id.trim().is_empty()
        || descriptor.descriptor_version != "1"
        || descriptor.packet_schema_ids.is_empty()
        || descriptor
            .packet_schema_ids
            .iter()
            .any(|schema_id| schema_id.trim().is_empty())
        || descriptor.fact_kinds.is_empty()
        || descriptor
            .fact_kinds
            .iter()
            .any(|fact_kind| fact_kind.trim().is_empty())
        || descriptor.intent_axes.is_empty()
    {
        return Err(AgentHookError::InvalidActivationConfig(format!(
            "provider manifest {} has an incomplete semanticFactsDescriptor",
            manifest.manifest_id
        )));
    }
    for (axis_index, intent_axis) in descriptor.intent_axes.iter().enumerate() {
        if intent_axis.axis.trim().is_empty()
            || intent_axis.terms.is_empty()
            || intent_axis.terms.iter().any(|term| term.trim().is_empty())
            || descriptor.intent_axes[..axis_index]
                .iter()
                .any(|previous| previous.axis == intent_axis.axis)
            || intent_axis
                .terms
                .iter()
                .enumerate()
                .any(|(term_index, term)| {
                    intent_axis.terms[..term_index]
                        .iter()
                        .any(|previous| previous.eq_ignore_ascii_case(term))
                })
            || intent_axis
                .roles
                .iter()
                .any(|role| !matches!(role.as_str(), "context" | "concept" | "symbol"))
        {
            return Err(AgentHookError::InvalidActivationConfig(format!(
                "provider manifest {} has an invalid semanticFactsDescriptor intent axis",
                manifest.manifest_id
            )));
        }
    }
    Ok(())
}
