//! Immutable Provider release inputs embedded by the ASP client.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

pub const PROVIDER_RELEASE_CATALOG_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-release-catalog";
pub const PROVIDER_RELEASE_CATALOG_SCHEMA_VERSION: &str = "1";
const BUILTIN_PROVIDER_RELEASE_CATALOG_TOML: &str =
    include_str!("../provider-release-catalog.v1.toml");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderReleaseRegistration {
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub repo: String,
    #[serde(rename = "version")]
    pub release_version: String,
    pub download_base_url: String,
    pub archive_prefix: Option<String>,
    pub archive_binary: Option<String>,
    pub require_native_binary: Option<bool>,
    pub supported_targets: Vec<String>,
    #[serde(default)]
    pub sha256_by_target: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderReleaseCatalog {
    pub schema_id: String,
    pub schema_version: String,
    pub releases: BTreeMap<String, ProviderReleaseRegistration>,
}

pub fn parse_provider_release_catalog(source: &str) -> Result<ProviderReleaseCatalog, String> {
    let catalog: ProviderReleaseCatalog = toml::from_str(source)
        .map_err(|error| format!("decode provider release catalog: {error}"))?;
    if catalog.schema_id != PROVIDER_RELEASE_CATALOG_SCHEMA_ID
        || catalog.schema_version != PROVIDER_RELEASE_CATALOG_SCHEMA_VERSION
    {
        return Err("provider release catalog schema identity mismatch".to_owned());
    }
    let mut provider_ids = std::collections::BTreeSet::new();
    let mut binaries = std::collections::BTreeSet::new();
    for (language_key, release) in &catalog.releases {
        if language_key != &release.language_id {
            return Err(format!(
                "provider release language key drift: key={language_key} languageId={}",
                release.language_id
            ));
        }
        if release.provider_id != format!("asp-{}", release.language_id)
            || release.binary != release.provider_id
        {
            return Err(format!(
                "provider release identity drift: languageId={}",
                release.language_id
            ));
        }
        if !provider_ids.insert(release.provider_id.as_str())
            || !binaries.insert(release.binary.as_str())
        {
            return Err("provider release identities must be unique".to_owned());
        }
        if release.release_version.is_empty()
            || release.repo.is_empty()
            || !release.download_base_url.starts_with("https://")
            || release.supported_targets.is_empty()
        {
            return Err(format!(
                "provider release descriptor is incomplete: languageId={}",
                release.language_id
            ));
        }
        let unique_targets = release
            .supported_targets
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        if unique_targets.len() != release.supported_targets.len() {
            return Err(format!(
                "provider release targets must be unique: languageId={}",
                release.language_id
            ));
        }
        let pinned_targets = release
            .sha256_by_target
            .keys()
            .collect::<std::collections::BTreeSet<_>>();
        if pinned_targets != unique_targets {
            return Err(format!(
                "provider release target checksum coverage mismatch: languageId={}",
                release.language_id
            ));
        }
        if release.sha256_by_target.values().any(|digest| {
            digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) {
            return Err(format!(
                "provider release target checksum is invalid: languageId={}",
                release.language_id
            ));
        }
    }
    Ok(catalog)
}

/// Decode the build-owned immutable Provider release distribution catalog.
pub fn builtin_provider_release_catalog() -> Result<ProviderReleaseCatalog, String> {
    parse_provider_release_catalog(BUILTIN_PROVIDER_RELEASE_CATALOG_TOML)
}
