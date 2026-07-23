use agent_semantic_rust_policy_types::{
    DownstreamPolicyReceipt, MemberPolicyRegistry, canonical_json_digest,
    validate_receipt_identity, validate_registry_identity,
};

use crate::command_path::normalized_relative_directory;
use crate::member_lookup::unique_member;
use crate::{VerificationInput, VerifiedMemberReceipt};

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
