// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::collections::{BTreeSet, HashMap};

use super::projection_validation::validate_selector;
use super::{WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot};

impl WorkspaceMemoryGeneration {
    pub(super) fn validate_evidence(&self) -> Result<(), String> {
        self.workspace_snapshot.validate()?;
        self.validate_identity()?;
        validate_digest("generationDigest", &self.generation_digest)?;
        validate_digest("providerSchemaDigest", &self.provider_schema_digest)?;
        validate_digest("moduleGraphDigest", &self.module_graph_digest)?;
        validate_digest("selectorSetDigest", &self.selector_set_digest)?;
        validate_digest("memoryBackendDigest", &self.memory_backend_digest)?;
        validate_digest(
            "workspaceSourceScopeGeneration",
            &self.workspace_source_scope_generation,
        )?;
        if self.workspace_source_scope_generation
            != agent_semantic_content_identity::workspace_source_scope_generation_digest(
                &self.project_resolutions,
            )?
        {
            return Err("workspace generation ProjectResolution evidence drift".to_owned());
        }
        if self.provider_schema_digest
            != agent_semantic_content_identity::project_resolution_schema_digest()
        {
            return Err("workspace generation provider schema authority drift".to_owned());
        }
        if let Some(binding) = &self.runtime_provider_execution_binding {
            binding.validate()?;
            if binding.source_snapshot_digest != self.source_snapshot.root_integrity_reference()?
                || binding.source_index_digest != self.module_graph_digest
            {
                return Err(
                    "workspace generation Runtime provider execution binding drift".to_owned(),
                );
            }
        }
        if self.workspace_snapshot.root_digest() != self.source_snapshot.root_digest
            || self.workspace_generation.root_digest != self.source_snapshot.root_digest
            || self.workspace_generation.root_depth != u32::from(self.root_depth[0])
            || self.workspace_generation.leaf_count
                != u64::try_from(self.source_snapshot.leaf_count)
                    .map_err(|_| "workspace generation leaf count overflow".to_owned())?
            || self.workspace_generation.owner_count
                != u64::try_from(self.owners.len())
                    .map_err(|_| "workspace generation owner count overflow".to_owned())?
        {
            return Err("workspace generation authority evidence drift".to_owned());
        }
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
            self.workspace_generation.clone(),
        )
        .map_err(|error| format!("workspace generation evidence is incomplete: {error}"))?;
        let mut unique_relations = BTreeSet::new();
        let owner_paths = self
            .owners
            .iter()
            .map(|owner| owner.owner_path.as_str())
            .collect::<BTreeSet<_>>();
        for owned in &self.relations {
            owned.relation.validate()?;
            if !owner_paths.contains(owned.owner_path.as_str()) {
                return Err(format!(
                    "workspace generation relation owner is absent: {}",
                    owned.owner_path.as_str()
                ));
            }
            if !unique_relations.insert(owned) {
                return Err(
                    "workspace generation contains a duplicate provider relation".to_owned(),
                );
            }
        }
        validate_owners(&self.owners)
    }
}

pub(super) fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    let Some(value) = digest.strip_prefix("blake3-256:") else {
        return Err(format!("{field} must use blake3-256"));
    };
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must contain a 64-character hex digest"));
    }
    Ok(())
}

pub(crate) fn validate_owners(owners: &[WorkspaceOwnerSnapshot]) -> Result<(), String> {
    let mut owner_paths = HashMap::with_capacity(owners.len());
    let mut selectors = HashMap::new();
    for (owner_index, owner) in owners.iter().enumerate() {
        validate_owner(owner)?;
        if owner_paths
            .insert(owner.owner_path.as_str(), owner_index)
            .is_some()
        {
            return Err(format!(
                "duplicate workspace owner path: {}",
                owner.owner_path
            ));
        }
        for selector in &owner.selectors {
            validate_selector(owner, selector)?;
            validate_selector_query_keys(selector)?;
            if selectors
                .insert(selector.selector.as_str(), owner.owner_path.as_str())
                .is_some()
            {
                return Err(format!(
                    "duplicate workspace selector identity: {}",
                    selector.selector
                ));
            }
        }
    }
    Ok(())
}

fn validate_selector_query_keys(selector: &super::WorkspaceSelectorSnapshot) -> Result<(), String> {
    if selector.query_keys.iter().any(|key| key.trim().is_empty())
        || selector
            .query_keys
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(format!(
            "workspace selector query keys are not canonical: selector={}",
            selector.selector
        ));
    }
    Ok(())
}

fn validate_owner(owner: &WorkspaceOwnerSnapshot) -> Result<(), String> {
    if owner.owner_path.trim().is_empty() {
        return Err("workspace owner path must be non-empty text".to_owned());
    }
    if let Some(authority) = &owner.authority
        && (authority.language_id.as_str().trim().is_empty()
            || authority.provider_id.as_str().trim().is_empty())
    {
        return Err("workspace owner authority must contain languageId and providerId".to_owned());
    }
    validate_digest("owner contentDigest", &owner.content_digest)?;
    let actual = format!("blake3-256:{}", blake3::hash(&owner.bytes).to_hex());
    if actual != owner.content_digest {
        return Err(format!(
            "workspace owner digest mismatch: ownerPath={}",
            owner.owner_path
        ));
    }
    if let Some(diagnostic) = &owner.native_syntax_diagnostic
        && (diagnostic.owner_path != owner.owner_path
            || diagnostic.content_digest != owner.content_digest
            || diagnostic.reason_kind != "source-syntax-unavailable"
            || diagnostic.message.trim().is_empty()
            || !owner.selectors.is_empty())
    {
        return Err("workspace owner native syntax diagnostic is invalid".to_owned());
    }
    Ok(())
}
