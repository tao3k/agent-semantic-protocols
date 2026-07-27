use std::collections::BTreeMap;

use serde::Deserialize;

/// One language/toolchain command family with responsibility-scoped argv prefixes.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientCommandProfileConfig {
    pub id: String,
    #[serde(default)]
    pub language_ids: Vec<String>,
    #[serde(default)]
    pub categories: BTreeMap<String, Vec<Vec<String>>>,
}

/// Typed reference from one hook rule to one profile responsibility category.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientCommandProfileRef {
    pub profile: String,
    pub category: String,
}

/// Resolve profile references into deterministic parser-owned argv prefixes.
pub fn expand_command_profile_prefixes(
    references: &[HookClientCommandProfileRef],
    profiles: &[HookClientCommandProfileConfig],
) -> Result<Vec<Vec<String>>, String> {
    let mut prefixes = Vec::new();
    for reference in references {
        let profile = profiles
            .iter()
            .find(|profile| profile.id == reference.profile)
            .ok_or_else(|| {
                format!(
                    "command profile reference `{}` uses missing profile `{}`",
                    reference.category, reference.profile
                )
            })?;
        let category = profile.categories.get(&reference.category).ok_or_else(|| {
            format!(
                "command profile `{}` has no category `{}`",
                reference.profile, reference.category
            )
        })?;
        for prefix in category {
            if !prefixes.contains(prefix) {
                prefixes.push(prefix.clone());
            }
        }
    }
    Ok(prefixes)
}
