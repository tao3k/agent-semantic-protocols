//! Serde admission rules for workspace IPC mutation identities.

use serde::Deserialize;

pub(super) fn deserialize_changed_paths<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let paths = Vec::<String>::deserialize(deserializer)?;
    if paths.is_empty() {
        return Err(serde::de::Error::custom(
            "changedPaths must contain at least one normalized path",
        ));
    }
    let mut unique = std::collections::BTreeSet::new();
    for path in &paths {
        if path.trim().is_empty() {
            return Err(serde::de::Error::custom(
                "changedPaths must not contain empty paths",
            ));
        }
        if !unique.insert(path) {
            return Err(serde::de::Error::custom(
                "changedPaths must not contain duplicate paths",
            ));
        }
    }
    Ok(paths)
}

pub(super) fn deserialize_mutation_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mutation_id = String::deserialize(deserializer)?;
    if mutation_id.trim().is_empty() {
        return Err(serde::de::Error::custom(
            "mutationId must be non-empty text",
        ));
    }
    Ok(mutation_id)
}
