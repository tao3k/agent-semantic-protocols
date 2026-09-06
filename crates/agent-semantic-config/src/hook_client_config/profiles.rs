// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Declarative command families expanded into parser-owned argument prefixes.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

macro_rules! command_profile_text_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Returns the exact declarative identifier text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
    };
}

command_profile_text_type!(
    HookClientCommandProfileId,
    "Identifier of a declarative command profile."
);
command_profile_text_type!(
    HookClientCommandCategory,
    "Responsibility category selected from a command profile."
);

/// One language/toolchain command family with responsibility-scoped argv prefixes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientCommandProfileConfig {
    pub id: String,
    #[serde(default)]
    pub language_ids: Vec<String>,
    #[serde(default)]
    pub categories: BTreeMap<String, Vec<Vec<String>>>,
}

/// Typed reference from one hook rule to one profile responsibility category.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientCommandProfileRef {
    /// Profile containing the selected responsibility category.
    pub profile: HookClientCommandProfileId,
    /// Responsibility category whose argument prefixes are selected.
    pub category: HookClientCommandCategory,
}

/// A repository-wide command family that is intentionally independent of any
/// language provider profile.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientCommandSetConfig {
    pub id: String,
    #[serde(default)]
    pub argv_prefix_any: Vec<Vec<String>>,
}

/// Resolve profile references into deterministic parser-owned argv prefixes.
pub fn expand_command_profile_prefixes(
    references: &[HookClientCommandProfileRef],
    profiles: &[HookClientCommandProfileConfig],
) -> Result<Vec<Vec<String>>, String> {
    let profile_index = profiles
        .iter()
        .map(|profile| (profile.id.as_str(), profile))
        .collect::<BTreeMap<_, _>>();
    let mut prefixes = Vec::new();
    let mut seen_prefixes = BTreeSet::new();
    for reference in references {
        let profile = profile_index
            .get(reference.profile.as_str())
            .copied()
            .ok_or_else(|| {
                format!(
                    "command profile reference `{}` uses missing profile `{}`",
                    reference.category, reference.profile
                )
            })?;
        let category = profile
            .categories
            .get(reference.category.as_str())
            .ok_or_else(|| {
                format!(
                    "command profile `{}` has no category `{}`",
                    reference.profile, reference.category
                )
            })?;
        for prefix in category {
            if seen_prefixes.insert(prefix.clone()) {
                prefixes.push(prefix.clone());
            }
        }
    }
    Ok(prefixes)
}

/// Resolve repository-wide command-set references into deterministic argv prefixes.
pub fn expand_command_set_prefixes(
    references: &[String],
    command_sets: &[HookClientCommandSetConfig],
) -> Result<Vec<Vec<String>>, String> {
    let command_set_index = command_sets
        .iter()
        .map(|command_set| (command_set.id.as_str(), command_set))
        .collect::<BTreeMap<_, _>>();
    let mut prefixes = Vec::new();
    let mut seen_prefixes = BTreeSet::new();
    for reference in references {
        let command_set = command_set_index
            .get(reference.as_str())
            .copied()
            .ok_or_else(|| format!("commandSetAny references missing command set `{reference}`"))?;
        for prefix in &command_set.argv_prefix_any {
            if seen_prefixes.insert(prefix.clone()) {
                prefixes.push(prefix.clone());
            }
        }
    }
    Ok(prefixes)
}
