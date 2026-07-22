//! Pure v1 receipt verification for Rust member build scripts.
//!
//! This crate starts no processes and writes no files. Canonical receipt-path
//! resolution and source snapshot collection are separate read-only adapters.

use std::path::{Component, Path};

use agent_semantic_rust_policy_types::{
    DownstreamPolicyReceipt, MemberPolicy, MemberPolicyRegistry, SourceSnapshot,
    canonical_json_digest, validate_receipt_identity, validate_registry_identity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationInput<'a> {
    pub package_name: &'a str,
    pub manifest_dir: &'a Path,
    pub workspace_root: &'a Path,
    pub observed_source_snapshot: &'a SourceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedMemberReceipt {
    pub package_name: String,
    pub package_directory: String,
    pub gate_label: String,
    pub source_snapshot: SourceSnapshot,
    pub policy_digest: String,
    pub execution_command_digest: String,
    pub cache_payload_digest: String,
}

pub fn verify_receipt_bytes(
    registry_bytes: &[u8],
    receipt_bytes: &[u8],
    input: VerificationInput<'_>,
) -> Result<VerifiedMemberReceipt, String> {
    let registry: MemberPolicyRegistry = serde_json::from_slice(registry_bytes)
        .map_err(|error| format!("invalid Rust member-policy registry v1 JSON: {error}"))?;
    validate_registry_identity(&registry)?;
    let member = unique_member(&registry, input.package_name)?;

    let receipt: DownstreamPolicyReceipt = serde_json::from_slice(receipt_bytes)
        .map_err(|error| format!("invalid Rust downstream-policy receipt v1 JSON: {error}"))?;
    validate_receipt_identity(&receipt)?;

    let expected_directory =
        normalized_relative_directory(input.workspace_root, input.manifest_dir)?;
    if member.package_directory != expected_directory {
        return Err(format!(
            "Rust member-policy registry directory mismatch for `{}`: expected `{expected_directory}`, got `{}`",
            input.package_name, member.package_directory
        ));
    }
    if receipt.package.name != input.package_name || receipt.package.directory != expected_directory
    {
        return Err(format!(
            "Rust downstream-policy receipt package mismatch for `{}`",
            input.package_name
        ));
    }
    if receipt.gate_label != member.gate_label {
        return Err(format!(
            "Rust downstream-policy receipt gate-label mismatch for `{}`",
            input.package_name
        ));
    }
    if &receipt.source_snapshot != input.observed_source_snapshot {
        return Err(format!(
            "Rust downstream-policy receipt source snapshot drift for `{}`",
            input.package_name
        ));
    }

    let expected_policy_digest = canonical_json_digest(member)?;
    if receipt.policy_digest != expected_policy_digest {
        return Err(format!(
            "Rust downstream-policy receipt policy drift for `{}`",
            input.package_name
        ));
    }
    let expected_execution_digest = canonical_json_digest(&registry.harness_execution)?;
    if receipt.execution_command_digest != expected_execution_digest {
        return Err(format!(
            "Rust downstream-policy receipt execution-command drift for `{}`",
            input.package_name
        ));
    }
    if receipt.dependency_baseline_packages != member.dependency_baseline_packages {
        return Err(format!(
            "Rust downstream-policy receipt dependency-baseline drift for `{}`",
            input.package_name
        ));
    }

    Ok(VerifiedMemberReceipt {
        package_name: receipt.package.name,
        package_directory: receipt.package.directory,
        gate_label: receipt.gate_label,
        source_snapshot: receipt.source_snapshot,
        policy_digest: receipt.policy_digest,
        execution_command_digest: receipt.execution_command_digest,
        cache_payload_digest: receipt.cache_payload_digest,
    })
}

pub fn prepare_command(registry: &MemberPolicyRegistry, package_name: &str) -> String {
    registry
        .harness_execution
        .prepare_command
        .iter()
        .map(|token| {
            if token == "{cargo-package-name}" {
                package_name
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|argument| {
            if argument == "{package}" {
                package_name
            } else {
                argument
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_member<'a>(
    registry: &'a MemberPolicyRegistry,
    package_name: &str,
) -> Result<&'a MemberPolicy, String> {
    let mut matches = registry
        .members
        .iter()
        .filter(|member| member.package == package_name);
    let member = matches
        .next()
        .ok_or_else(|| format!("no Rust member policy registered for `{package_name}`"))?;
    if matches.next().is_some() {
        return Err(format!(
            "duplicate Rust member policies registered for `{package_name}`"
        ));
    }
    Ok(member)
}

fn normalized_relative_directory(
    workspace_root: &Path,
    manifest_dir: &Path,
) -> Result<String, String> {
    let relative = manifest_dir.strip_prefix(workspace_root).map_err(|_| {
        format!(
            "CARGO_MANIFEST_DIR `{}` is outside workspace root `{}`",
            manifest_dir.display(),
            workspace_root.display()
        )
    })?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => segments.push(segment.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "invalid member package directory `{}`",
                    relative.display()
                ));
            }
        }
    }
    if segments.is_empty() {
        return Ok(".".to_string());
    }
    Ok(segments.join("/"))
}
