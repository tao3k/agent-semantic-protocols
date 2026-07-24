use agent_semantic_rust_policy_types::{MemberPolicy, MemberPolicyRegistry};

pub(crate) fn unique_member<'a>(
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
