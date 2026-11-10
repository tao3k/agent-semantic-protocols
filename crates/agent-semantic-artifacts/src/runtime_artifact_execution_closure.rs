// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical typed documents forming the immutable Runtime execution closure.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_slots::RuntimeArtifactBundleBinding;

pub const RUNTIME_ARTIFACT_EXECUTION_CLOSURE_MEMBER_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-artifact-execution-closure-member";
pub const RUNTIME_ARTIFACT_EXECUTION_CLOSURE_MEMBER_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeArtifactExecutionClosureMemberKind {
    ProviderRegistration,
    ProviderArtifactSet,
    EvaluatorPolicy,
    EvaluatorAbi,
    SchemaBundle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeArtifactExecutionClosureMember<Entry> {
    pub schema_id: String,
    pub schema_version: String,
    pub member_kind: RuntimeArtifactExecutionClosureMemberKind,
    pub entries: Vec<Entry>,
}

impl<Entry> RuntimeArtifactExecutionClosureMember<Entry> {
    pub fn new(
        member_kind: RuntimeArtifactExecutionClosureMemberKind,
        entries: Vec<Entry>,
    ) -> Self {
        Self {
            schema_id: RUNTIME_ARTIFACT_EXECUTION_CLOSURE_MEMBER_SCHEMA_ID.into(),
            schema_version: RUNTIME_ARTIFACT_EXECUTION_CLOSURE_MEMBER_SCHEMA_VERSION.into(),
            member_kind,
            entries,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRegistrationClosureEntry {
    pub provider_id: String,
    pub language_id: String,
    pub registration_digest: Blake3ContentDigest,
    pub artifact_member: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderArtifactClosureEntry {
    pub provider_id: String,
    pub artifact_member: String,
    pub artifact_digest: Blake3ContentDigest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedRuntimeDigestClosureEntry {
    pub id: String,
    pub digest: Blake3ContentDigest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageSchemaClosureEntry {
    pub language_id: String,
    pub schema_digest: Blake3ContentDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactExecutionClosure {
    pub provider_registration:
        RuntimeArtifactExecutionClosureMember<ProviderRegistrationClosureEntry>,
    pub provider_artifact_set: RuntimeArtifactExecutionClosureMember<ProviderArtifactClosureEntry>,
    pub evaluator_policy: RuntimeArtifactExecutionClosureMember<NamedRuntimeDigestClosureEntry>,
    pub evaluator_abi: RuntimeArtifactExecutionClosureMember<NamedRuntimeDigestClosureEntry>,
    pub schema_bundle: RuntimeArtifactExecutionClosureMember<LanguageSchemaClosureEntry>,
}

impl RuntimeArtifactExecutionClosure {
    pub fn from_runtime_bundle_members(
        members: &BTreeMap<String, Blake3ContentDigest>,
        evaluator_policy_entries: Vec<NamedRuntimeDigestClosureEntry>,
        evaluator_abi_entries: Vec<NamedRuntimeDigestClosureEntry>,
        schema_bundle_entries: Vec<LanguageSchemaClosureEntry>,
    ) -> Result<Self, String> {
        let mut provider_registration_entries = Vec::new();
        let mut provider_artifact_entries = Vec::new();
        for registration in agent_semantic_provider_protocol::builtin_provider_registrations()? {
            let Some(artifact_digest) = members.get(&registration.provider_id) else {
                continue;
            };
            provider_registration_entries.push(ProviderRegistrationClosureEntry {
                provider_id: registration.provider_id.clone(),
                language_id: registration.language_id,
                registration_digest: Blake3ContentDigest::from_bytes(
                    &serde_json::to_vec(&registration.registration)
                        .map_err(|error| format!("encode provider registration: {error}"))?,
                ),
                artifact_member: registration.provider_id.clone(),
            });
            provider_artifact_entries.push(ProviderArtifactClosureEntry {
                provider_id: registration.provider_id.clone(),
                artifact_member: registration.provider_id,
                artifact_digest: artifact_digest.clone(),
            });
        }
        provider_registration_entries
            .sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        provider_artifact_entries.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        let closure = Self {
            provider_registration: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::ProviderRegistration,
                provider_registration_entries,
            ),
            provider_artifact_set: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::ProviderArtifactSet,
                provider_artifact_entries,
            ),
            evaluator_policy: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::EvaluatorPolicy,
                evaluator_policy_entries,
            ),
            evaluator_abi: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::EvaluatorAbi,
                evaluator_abi_entries,
            ),
            schema_bundle: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::SchemaBundle,
                schema_bundle_entries,
            ),
        };
        closure.validate_against_bundle(members)?;
        Ok(closure)
    }

    pub fn from_materialized_members(
        root: &Path,
        members: &BTreeMap<String, Blake3ContentDigest>,
    ) -> Result<Self, String> {
        let closure = Self {
            provider_registration: decode_member(root, members, "provider-registration.json")?,
            provider_artifact_set: decode_member(root, members, "provider-artifact-set")?,
            evaluator_policy: decode_member(root, members, "evaluator-policy.json")?,
            evaluator_abi: decode_member(root, members, "evaluator-abi.json")?,
            schema_bundle: decode_member(root, members, "schema-bundle.json")?,
        };
        closure.validate()?;
        Ok(closure)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_member(
            &self.provider_registration,
            RuntimeArtifactExecutionClosureMemberKind::ProviderRegistration,
            true,
        )?;
        validate_member(
            &self.provider_artifact_set,
            RuntimeArtifactExecutionClosureMemberKind::ProviderArtifactSet,
            true,
        )?;
        validate_member(
            &self.evaluator_policy,
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorPolicy,
            false,
        )?;
        validate_member(
            &self.evaluator_abi,
            RuntimeArtifactExecutionClosureMemberKind::EvaluatorAbi,
            false,
        )?;
        validate_member(
            &self.schema_bundle,
            RuntimeArtifactExecutionClosureMemberKind::SchemaBundle,
            false,
        )?;
        let registrations = self
            .provider_registration
            .entries
            .iter()
            .map(|entry| (&entry.provider_id, &entry.artifact_member))
            .collect::<Vec<_>>();
        let artifacts = self
            .provider_artifact_set
            .entries
            .iter()
            .map(|entry| (&entry.provider_id, &entry.artifact_member))
            .collect::<Vec<_>>();
        if registrations != artifacts {
            return Err(
                "reasonKind=runtime-execution-closure-provider-coverage-mismatch".to_owned(),
            );
        }
        Ok(())
    }

    pub fn materialized_members(&self) -> Result<BTreeMap<&'static str, Vec<u8>>, String> {
        self.validate()?;
        Ok(BTreeMap::from([
            (
                "provider-registration.json",
                encode(&self.provider_registration)?,
            ),
            (
                "provider-artifact-set",
                encode(&self.provider_artifact_set)?,
            ),
            ("evaluator-policy.json", encode(&self.evaluator_policy)?),
            ("evaluator-abi.json", encode(&self.evaluator_abi)?),
            ("schema-bundle.json", encode(&self.schema_bundle)?),
        ]))
    }

    pub fn binding(&self) -> Result<RuntimeArtifactBundleBinding, String> {
        let members = self.materialized_members()?;
        Ok(RuntimeArtifactBundleBinding::new(
            digest_member(&members, "provider-registration.json")?,
            digest_member(&members, "provider-artifact-set")?,
            digest_member(&members, "evaluator-policy.json")?,
            digest_member(&members, "evaluator-abi.json")?,
            digest_member(&members, "schema-bundle.json")?,
        ))
    }

    pub fn validate_against_bundle(
        &self,
        members: &BTreeMap<String, Blake3ContentDigest>,
    ) -> Result<(), String> {
        self.validate_provider_registrations(None)?;
        self.validate_provider_artifacts(members)
    }

    pub(crate) fn validate_provider_replacement_predecessor(
        &self,
        members: &BTreeMap<String, Blake3ContentDigest>,
        replaced_provider_id: &str,
    ) -> Result<(), String> {
        if replaced_provider_id.is_empty() {
            return Err(
                "reasonKind=runtime-execution-closure-provider-replacement-empty".to_owned(),
            );
        }
        self.validate_provider_registrations(Some(replaced_provider_id))?;
        self.validate_provider_artifacts(members)
    }

    pub(crate) fn validate_binary_replacement_predecessor(
        &self,
        members: &BTreeMap<String, Blake3ContentDigest>,
    ) -> Result<(), String> {
        self.validate_provider_registrations_for_binary_replacement()?;
        self.validate_provider_artifacts(members)
    }

    fn validate_provider_registrations(
        &self,
        replaced_provider_id: Option<&str>,
    ) -> Result<(), String> {
        self.validate()?;
        let registrations = agent_semantic_provider_protocol::builtin_provider_registrations()?;
        let registrations = registrations
            .into_iter()
            .map(|registration| (registration.provider_id.clone(), registration))
            .collect::<BTreeMap<_, _>>();
        for entry in &self.provider_registration.entries {
            let registration = registrations.get(&entry.provider_id).ok_or_else(|| {
                format!(
                    "reasonKind=runtime-execution-closure-provider-registration-unknown providerId={}",
                    entry.provider_id
                )
            })?;
            let identity_matches = registration.language_id == entry.language_id
                && registration.provider_id == entry.artifact_member;
            let registration_digest = Blake3ContentDigest::from_bytes(
                &serde_json::to_vec(&registration.registration)
                    .map_err(|error| format!("encode built-in provider registration: {error}"))?,
            );
            if !identity_matches
                || (registration_digest != entry.registration_digest
                    && replaced_provider_id != Some(entry.provider_id.as_str()))
            {
                return Err(format!(
                    "reasonKind=runtime-execution-closure-provider-registration-drift providerId={}",
                    entry.provider_id
                ));
            }
        }
        Ok(())
    }

    fn validate_provider_registrations_for_binary_replacement(&self) -> Result<(), String> {
        self.validate()?;
        let registrations = agent_semantic_provider_protocol::builtin_provider_registrations()?
            .into_iter()
            .map(|registration| (registration.provider_id.clone(), registration))
            .collect::<BTreeMap<_, _>>();
        for entry in &self.provider_registration.entries {
            let registration = registrations.get(&entry.provider_id).ok_or_else(|| {
                format!(
                    "reasonKind=runtime-execution-closure-provider-registration-unknown providerId={}",
                    entry.provider_id
                )
            })?;
            if registration.language_id != entry.language_id
                || registration.provider_id != entry.artifact_member
            {
                return Err(format!(
                    "reasonKind=runtime-execution-closure-provider-registration-drift providerId={}",
                    entry.provider_id
                ));
            }
        }
        Ok(())
    }

    fn validate_provider_artifacts(
        &self,
        members: &BTreeMap<String, Blake3ContentDigest>,
    ) -> Result<(), String> {
        self.validate()?;
        for entry in &self.provider_artifact_set.entries {
            let observed = members.get(&entry.artifact_member).ok_or_else(|| {
                format!(
                    "reasonKind=runtime-execution-closure-provider-artifact-missing member={}",
                    entry.artifact_member
                )
            })?;
            if observed != &entry.artifact_digest {
                return Err(format!(
                    "reasonKind=runtime-execution-closure-provider-artifact-drift member={}",
                    entry.artifact_member
                ));
            }
        }
        Ok(())
    }
}

fn decode_member<Entry: for<'de> Deserialize<'de>>(
    root: &Path,
    members: &BTreeMap<String, Blake3ContentDigest>,
    name: &'static str,
) -> Result<RuntimeArtifactExecutionClosureMember<Entry>, String> {
    if !members.contains_key(name) {
        return Err(format!(
            "reasonKind=runtime-execution-closure-member-missing member={name}"
        ));
    }
    let bytes = std::fs::read(root.join(name))
        .map_err(|error| format!("read Runtime execution closure member `{name}`: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!("reasonKind=runtime-execution-closure-member-invalid member={name} error={error}")
    })
}

fn validate_member<Entry: Serialize>(
    member: &RuntimeArtifactExecutionClosureMember<Entry>,
    expected_kind: RuntimeArtifactExecutionClosureMemberKind,
    allow_empty: bool,
) -> Result<(), String> {
    if member.schema_id != RUNTIME_ARTIFACT_EXECUTION_CLOSURE_MEMBER_SCHEMA_ID
        || member.schema_version != RUNTIME_ARTIFACT_EXECUTION_CLOSURE_MEMBER_SCHEMA_VERSION
        || member.member_kind != expected_kind
        || (!allow_empty && member.entries.is_empty())
    {
        return Err("reasonKind=runtime-execution-closure-member-invalid".to_owned());
    }
    let encoded = member
        .entries
        .iter()
        .map(|entry| serde_json::to_vec(entry))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("encode Runtime execution closure entry: {error}"))?;
    if !encoded.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err("reasonKind=runtime-execution-closure-member-order-invalid".to_owned());
    }
    Ok(())
}

fn encode<Entry: Serialize>(
    member: &RuntimeArtifactExecutionClosureMember<Entry>,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(member)
        .map_err(|error| format!("encode Runtime artifact execution closure member: {error}"))
}

fn digest_member(
    members: &BTreeMap<&'static str, Vec<u8>>,
    name: &'static str,
) -> Result<Blake3ContentDigest, String> {
    members
        .get(name)
        .map(|bytes| Blake3ContentDigest::from_bytes(bytes))
        .ok_or_else(|| format!("Runtime artifact execution closure omitted `{name}`"))
}

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_execution_closure.rs"]
mod tests;
