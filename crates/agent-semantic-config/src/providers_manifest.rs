// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed `providers.toml` package manifest replacing `.gitmodules`.
//!
//! External language providers (`languages/*`) and optional analyzers
//! (`analyzers/*`) communicate with ASP across process/gRPC boundaries rather
//! than Rust compile-time coupling. Declaring them in `providers.toml` keeps
//! Cargo git consumers (such as Marlin) completely free of submodule fetches
//! while preserving one-command multi-repo sync and local co-development.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

/// Canonical filename for the workspace provider package manifest.
pub const PROVIDERS_MANIFEST_FILE: &str = "providers.toml";

/// Supported schema version for `providers.toml`.
pub const PROVIDERS_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Domain newtype identifying a package entry in `providers.toml`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProviderPackageId(String);

impl ProviderPackageId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProvidersManifestError> {
        let value = value.into();
        if value.is_empty()
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(ProvidersManifestError::InvalidPackageId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderPackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Domain newtype holding a full 40-character lowercase Git commit SHA.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProviderGitRev(String);

impl ProviderGitRev {
    pub fn new(value: impl Into<String>) -> Result<Self, ProvidersManifestError> {
        let value = value.into();
        if value.len() != 40
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ProvidersManifestError::InvalidGitRev(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderGitRev {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Classification of an external package managed by `providers.toml`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderPackageKind {
    LanguageProvider,
    Analyzer,
    Docs,
}

/// Runtime boundary used between ASP and a managed external package.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTransportKind {
    NativeOrGrpc,
    Grpc,
    Http,
    LocalLibrary,
    Docs,
}

/// Repository access visibility for profile/CI planning.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderVisibility {
    Public,
    Private,
}

/// One package entry declared in `providers.toml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderPackageSpec {
    pub kind: ProviderPackageKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub repo: String,
    pub rev: ProviderGitRev,
    pub path: String,
    pub transport: ProviderTransportKind,
    pub visibility: ProviderVisibility,
    #[serde(default)]
    pub optional: bool,
}

/// Root typed TOML envelope for `providers.toml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvidersManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub profiles: BTreeMap<String, Vec<ProviderPackageId>>,
    pub packages: BTreeMap<ProviderPackageId, ProviderPackageSpec>,
}

/// Deterministic error returned when loading, validating, or selecting from `providers.toml`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvidersManifestError {
    Io(String),
    Parse(String),
    UnsupportedSchemaVersion(u32),
    InvalidPackageId(String),
    InvalidGitRev(String),
    InvalidRepoUrl { package: String, repo: String },
    InvalidRelativePath { package: String, path: String },
    DuplicatePackagePath { path: String },
    MissingLanguageForProvider(String),
    UnknownProfile(String),
    UnknownPackage(String),
}

impl fmt::Display for ProvidersManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(formatter, "providers manifest I/O error: {message}"),
            Self::Parse(message) => write!(formatter, "providers manifest TOML error: {message}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported providers.toml schema_version: {version}"
                )
            }
            Self::InvalidPackageId(id) => write!(formatter, "invalid provider package id: `{id}`"),
            Self::InvalidGitRev(rev) => {
                write!(formatter, "invalid 40-char lowercase git rev: `{rev}`")
            }
            Self::InvalidRepoUrl { package, repo } => {
                write!(
                    formatter,
                    "package `{package}` has invalid repo URL: `{repo}`"
                )
            }
            Self::InvalidRelativePath { package, path } => {
                write!(
                    formatter,
                    "package `{package}` has unsafe relative path: `{path}`"
                )
            }
            Self::DuplicatePackagePath { path } => {
                write!(formatter, "duplicate provider checkout path: `{path}`")
            }
            Self::MissingLanguageForProvider(package) => {
                write!(
                    formatter,
                    "language-provider package `{package}` requires a `language` field"
                )
            }
            Self::UnknownProfile(profile) => {
                write!(formatter, "unknown provider profile: `{profile}`")
            }
            Self::UnknownPackage(package) => {
                write!(formatter, "unknown provider package: `{package}`")
            }
        }
    }
}

impl std::error::Error for ProvidersManifestError {}

impl ProvidersManifest {
    /// Parse and validate a `providers.toml` document.
    pub fn parse_str(source: &str) -> Result<Self, ProvidersManifestError> {
        let manifest: Self = toml::from_str(source)
            .map_err(|error| ProvidersManifestError::Parse(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate all fail-closed invariants on the manifest.
    pub fn validate(&self) -> Result<(), ProvidersManifestError> {
        if self.schema_version != PROVIDERS_MANIFEST_SCHEMA_VERSION {
            return Err(ProvidersManifestError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        let mut seen_paths = BTreeSet::new();
        for (id, spec) in &self.packages {
            ProviderPackageId::new(id.as_str())?;
            ProviderGitRev::new(spec.rev.as_str())?;
            if !(spec.repo.starts_with("https://") || spec.repo.starts_with("ssh://")) {
                return Err(ProvidersManifestError::InvalidRepoUrl {
                    package: id.as_str().to_owned(),
                    repo: spec.repo.clone(),
                });
            }
            validate_safe_relative_path(id.as_str(), &spec.path)?;
            if !seen_paths.insert(spec.path.as_str()) {
                return Err(ProvidersManifestError::DuplicatePackagePath {
                    path: spec.path.clone(),
                });
            }
            if spec.kind == ProviderPackageKind::LanguageProvider
                && spec
                    .language
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            {
                return Err(ProvidersManifestError::MissingLanguageForProvider(
                    id.as_str().to_owned(),
                ));
            }
        }
        for members in self.profiles.values() {
            for member in members {
                if !self.packages.contains_key(member) {
                    return Err(ProvidersManifestError::UnknownPackage(
                        member.as_str().to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Resolve a deterministic ordered list of packages from `--profile` and/or `--package`.
    pub fn select_packages(
        &self,
        profile: Option<&str>,
        explicit_packages: &[String],
    ) -> Result<Vec<(&ProviderPackageId, &ProviderPackageSpec)>, ProvidersManifestError> {
        let mut selected_ids = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(profile_name) = profile {
            let members = self
                .profiles
                .get(profile_name)
                .ok_or_else(|| ProvidersManifestError::UnknownProfile(profile_name.to_owned()))?;
            for member in members {
                if seen.insert(member.as_str().to_owned()) {
                    selected_ids.push(member.clone());
                }
            }
        }
        for name in explicit_packages {
            let id = ProviderPackageId::new(name.as_str())?;
            if !self.packages.contains_key(&id) {
                return Err(ProvidersManifestError::UnknownPackage(name.clone()));
            }
            if seen.insert(name.clone()) {
                selected_ids.push(id);
            }
        }
        if selected_ids.is_empty() && profile.is_none() && explicit_packages.is_empty() {
            for id in self.packages.keys() {
                selected_ids.push(id.clone());
            }
        }
        selected_ids
            .into_iter()
            .map(|id| {
                self.packages
                    .get_key_value(&id)
                    .ok_or_else(|| ProvidersManifestError::UnknownPackage(id.as_str().to_owned()))
            })
            .collect()
    }
}

fn validate_safe_relative_path(package: &str, raw: &str) -> Result<(), ProvidersManifestError> {
    let path = Path::new(raw);
    if raw.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ProvidersManifestError::InvalidRelativePath {
            package: package.to_owned(),
            path: raw.to_owned(),
        });
    }
    Ok(())
}

/// Load and validate `providers.toml` from `workspace_root`.
pub fn load_providers_manifest(
    workspace_root: &Path,
) -> Result<ProvidersManifest, ProvidersManifestError> {
    let path = workspace_root.join(PROVIDERS_MANIFEST_FILE);
    let source = std::fs::read_to_string(&path)
        .map_err(|error| ProvidersManifestError::Io(format!("read {}: {error}", path.display())))?;
    ProvidersManifest::parse_str(&source)
}

/// Render and atomically write a validated `providers.toml` to `workspace_root`.
pub fn save_providers_manifest(
    workspace_root: &Path,
    manifest: &ProvidersManifest,
) -> Result<(), ProvidersManifestError> {
    manifest.validate()?;
    let body = toml::to_string_pretty(manifest)
        .map_err(|error| ProvidersManifestError::Parse(error.to_string()))?;
    let header = "# SPDX-FileCopyrightText: 2026 tao3k team and Contributors\n#\n# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later\n\n";
    let path = workspace_root.join(PROVIDERS_MANIFEST_FILE);
    std::fs::write(&path, format!("{header}{body}"))
        .map_err(|error| ProvidersManifestError::Io(format!("write {}: {error}", path.display())))
}
