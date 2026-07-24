use crate::{
    DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID, DownstreamPolicyReceipt, MEMBER_POLICY_REGISTRY_SCHEMA_ID,
    MemberPolicyRegistry, SCHEMA_VERSION,
};

pub fn validate_registry_identity(registry: &MemberPolicyRegistry) -> Result<(), String> {
    if registry.schema_id != MEMBER_POLICY_REGISTRY_SCHEMA_ID
        || registry.schema_version != SCHEMA_VERSION
    {
        return Err("invalid Rust member-policy registry v1 identity".to_string());
    }
    if registry.members.is_empty() {
        return Err("Rust member-policy registry v1 has no members".to_string());
    }
    for (index, member) in registry.members.iter().enumerate() {
        if member.package.is_empty()
            || member.package_directory.is_empty()
            || member.gate_label.is_empty()
        {
            return Err(format!(
                "invalid empty member identity at registry index {index}"
            ));
        }
        if registry.members[..index]
            .iter()
            .any(|candidate| candidate.package == member.package)
        {
            return Err(format!("duplicate Rust member policy `{}`", member.package));
        }
    }
    Ok(())
}

pub fn validate_receipt_identity(receipt: &DownstreamPolicyReceipt) -> Result<(), String> {
    if receipt.schema_id != DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
    {
        return Err("invalid Rust downstream-policy receipt v1 identity".to_string());
    }
    for (label, digest) in [
        (
            "source_snapshot.digest",
            receipt.source_snapshot.digest.as_str(),
        ),
        ("policy_digest", receipt.policy_digest.as_str()),
        (
            "execution_command_digest",
            receipt.execution_command_digest.as_str(),
        ),
        (
            "cache_payload_digest",
            receipt.cache_payload_digest.as_str(),
        ),
    ] {
        validate_blake3_digest(label, digest)?;
    }
    Ok(())
}

fn validate_blake3_digest(label: &str, digest: &str) -> Result<(), String> {
    let Some(hex) = digest.strip_prefix("blake3:") else {
        return Err(format!("{label} is not a blake3 digest"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} is not lowercase blake3 hex"));
    }
    Ok(())
}
