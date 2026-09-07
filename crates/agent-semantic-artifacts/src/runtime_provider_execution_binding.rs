// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable workspace execution binding to one verified Runtime bundle.

use serde::Serialize;

pub const RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-provider-execution-binding";
pub const RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProviderExecutionBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: String,
    pub project_id: String,
    pub workspace_id: String,
    pub runtime_bundle_digest: String,
    pub schema_bundle_digest: String,
    pub workspace_closure_digest: String,
    pub source_snapshot_digest: String,
    pub source_index_digest: String,
}

impl RuntimeProviderExecutionBinding {
    pub fn build(
        project_id: String,
        workspace_id: String,
        runtime_bundle_digest: String,
        schema_bundle_digest: String,
        workspace_closure_digest: String,
        source_snapshot_digest: String,
        source_index_digest: String,
    ) -> Result<Self, String> {
        let mut binding = Self {
            schema_id: RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_ID.to_owned(),
            schema_version: RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_VERSION.to_owned(),
            generation: String::new(),
            project_id,
            workspace_id,
            runtime_bundle_digest,
            schema_bundle_digest,
            workspace_closure_digest,
            source_snapshot_digest,
            source_index_digest,
        };
        binding.generation = binding.derived_generation()?;
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_ID
            || self.schema_version != RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_VERSION
        {
            return Err("runtime provider execution binding schema identity mismatch".to_owned());
        }
        if self
            .project_id
            .strip_prefix("repo-")
            .is_none_or(str::is_empty)
        {
            return Err(
                "runtime provider execution binding projectId is not a canonical RepoId".to_owned(),
            );
        }
        if self
            .workspace_id
            .strip_prefix("workspace-")
            .is_none_or(str::is_empty)
        {
            return Err(
                "runtime provider execution binding workspaceId is not a canonical WorkspaceId"
                    .to_owned(),
            );
        }
        for (field, digest) in [
            ("runtimeBundleDigest", &self.runtime_bundle_digest),
            ("schemaBundleDigest", &self.schema_bundle_digest),
            ("workspaceClosureDigest", &self.workspace_closure_digest),
            ("sourceSnapshotDigest", &self.source_snapshot_digest),
            ("sourceIndexDigest", &self.source_index_digest),
        ] {
            validate_digest(field, digest)?;
        }
        if self.generation != self.derived_generation()? {
            return Err("runtime provider execution binding generation drift".to_owned());
        }
        Ok(())
    }

    fn derived_generation(&self) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Identity<'a> {
            project_id: &'a str,
            workspace_id: &'a str,
            runtime_bundle_digest: &'a str,
            schema_bundle_digest: &'a str,
            workspace_closure_digest: &'a str,
            source_snapshot_digest: &'a str,
            source_index_digest: &'a str,
        }
        let bytes = serde_json::to_vec(&Identity {
            project_id: &self.project_id,
            workspace_id: &self.workspace_id,
            runtime_bundle_digest: &self.runtime_bundle_digest,
            schema_bundle_digest: &self.schema_bundle_digest,
            workspace_closure_digest: &self.workspace_closure_digest,
            source_snapshot_digest: &self.source_snapshot_digest,
            source_index_digest: &self.source_index_digest,
        })
        .map_err(|error| format!("encode runtime provider execution binding: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
    }
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    let Some((algorithm, value)) = digest.split_once(':') else {
        return Err(format!(
            "runtime provider execution binding {field} is not a digest"
        ));
    };
    if !matches!(algorithm, "blake3-256" | "sha256")
        || value.len() != 64
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!(
            "runtime provider execution binding {field} is invalid"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn digest(byte: char) -> String {
        format!("blake3-256:{}", byte.to_string().repeat(64))
    }

    fn binding(runtime_bundle_digest: String) -> super::RuntimeProviderExecutionBinding {
        super::RuntimeProviderExecutionBinding::build(
            "repo-project".to_owned(),
            "workspace-project".to_owned(),
            runtime_bundle_digest,
            digest('b'),
            digest('c'),
            digest('d'),
            digest('e'),
        )
        .expect("binding")
    }

    #[test]
    fn execution_identity_changes_with_the_active_runtime_bundle() {
        assert_ne!(
            binding(digest('a')).generation,
            binding(digest('f')).generation
        );
    }

    #[test]
    fn activation_sequence_cannot_substitute_for_runtime_content_identity() {
        let error = super::RuntimeProviderExecutionBinding::build(
            "repo-project".to_owned(),
            "workspace-project".to_owned(),
            "84".to_owned(),
            digest('b'),
            digest('c'),
            digest('d'),
            digest('e'),
        )
        .expect_err("publication sequence is not a content digest");
        assert!(error.contains("runtimeBundleDigest"), "{error}");
    }
}
