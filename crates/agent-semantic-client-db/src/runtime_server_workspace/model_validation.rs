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
            != agent_semantic_runtime::workspace_source_scope_generation_digest(
                &self.project_resolutions,
            )?
        {
            return Err("workspace generation ProjectResolution evidence drift".to_owned());
        }
        if self.provider_schema_digest != agent_semantic_runtime::project_resolution_schema_digest()
        {
            return Err("workspace generation provider schema authority drift".to_owned());
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
        for relation in &self.relations {
            relation.validate()?;
            if !unique_relations.insert(relation) {
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

fn validate_owner(owner: &WorkspaceOwnerSnapshot) -> Result<(), String> {
    if owner.owner_path.trim().is_empty() {
        return Err("workspace owner path must be non-empty text".to_owned());
    }
    validate_digest("owner contentDigest", &owner.content_digest)?;
    let actual = format!("blake3-256:{}", blake3::hash(&owner.bytes).to_hex());
    if actual != owner.content_digest {
        return Err(format!(
            "workspace owner digest mismatch: ownerPath={}",
            owner.owner_path
        ));
    }
    Ok(())
}
