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
            if registration.language_id != entry.language_id
                || Blake3ContentDigest::from_bytes(
                    &serde_json::to_vec(&registration.registration).map_err(|error| {
                        format!("encode built-in provider registration: {error}")
                    })?,
                ) != entry.registration_digest
            {
                return Err(format!(
                    "reasonKind=runtime-execution-closure-provider-registration-drift providerId={}",
                    entry.provider_id
                ));
            }
        }
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
mod tests {
    use super::*;

    fn digest(byte: u8) -> Blake3ContentDigest {
        Blake3ContentDigest::from_bytes(&[byte])
    }

    fn closure() -> RuntimeArtifactExecutionClosure {
        RuntimeArtifactExecutionClosure {
            provider_registration: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::ProviderRegistration,
                vec![ProviderRegistrationClosureEntry {
                    provider_id: "asp-rust".into(),
                    language_id: "rust".into(),
                    registration_digest: digest(1),
                    artifact_member: "asp-rust".into(),
                }],
            ),
            provider_artifact_set: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::ProviderArtifactSet,
                vec![ProviderArtifactClosureEntry {
                    provider_id: "asp-rust".into(),
                    artifact_member: "asp-rust".into(),
                    artifact_digest: digest(2),
                }],
            ),
            evaluator_policy: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::EvaluatorPolicy,
                vec![NamedRuntimeDigestClosureEntry {
                    id: "query-admission".into(),
                    digest: digest(4),
                }],
            ),
            evaluator_abi: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::EvaluatorAbi,
                vec![NamedRuntimeDigestClosureEntry {
                    id: "query-playbook-v1".into(),
                    digest: digest(5),
                }],
            ),
            schema_bundle: RuntimeArtifactExecutionClosureMember::new(
                RuntimeArtifactExecutionClosureMemberKind::SchemaBundle,
                vec![LanguageSchemaClosureEntry {
                    language_id: "rust".into(),
                    schema_digest: digest(6),
                }],
            ),
        }
    }

    #[test]
    fn complete_closure_mints_the_five_member_bundle_binding() {
        let closure = closure();
        closure.validate().expect("complete closure");
        assert_eq!(closure.materialized_members().unwrap().len(), 5);
        assert_eq!(closure.binding().unwrap(), closure.binding().unwrap());
    }

    #[test]
    fn provider_registration_without_its_artifact_is_rejected() {
        let mut closure = closure();
        closure.provider_artifact_set.entries[0].provider_id = "asp-python".into();
        assert_eq!(
            closure.validate(),
            Err("reasonKind=runtime-execution-closure-provider-coverage-mismatch".to_owned())
        );
    }

    #[test]
    fn explicit_empty_provider_pair_is_a_valid_bootstrap_closure() {
        let mut closure = closure();
        closure.provider_registration.entries.clear();
        closure.provider_artifact_set.entries.clear();
        closure
            .validate()
            .expect("empty provider bootstrap closure");
        assert_eq!(closure.materialized_members().unwrap().len(), 5);
    }

    #[test]
    fn activation_sequence_cannot_fill_an_empty_member() {
        let mut closure = closure();
        closure.evaluator_abi.entries.clear();
        assert_eq!(
            closure.validate(),
            Err("reasonKind=runtime-execution-closure-member-invalid".to_owned())
        );
    }
}
