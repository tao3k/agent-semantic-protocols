//! Content-addressed Git and artifact paths for provider live corpora.

use crate::state_core::RemoteUrl;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};

const BLAKE3_256: &str = "blake3-256";
pub const LIVE_CORPUS_ARTIFACT_SCHEMA_ID: &str = "agent.semantic-protocols.live-corpus-artifact";
pub const LIVE_CORPUS_ARTIFACT_SCHEMA_VERSION: &str = "1";

/// Canonical repository paths derived from a `gix`-normalized remote URL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCorpusGitRepositoryPaths {
    pub canonical_remote_identity: String,
    pub remote_digest: String,
    pub repository_dir: PathBuf,
    pub ghq_alias_path: PathBuf,
}

/// Result of explicitly synchronizing one pinned checkout through `gix`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCorpusGitCheckoutSync {
    pub checkout_dir: PathBuf,
    pub reused: bool,
}

/// Immutable live-corpus artifact paths derived from source and provider facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCorpusArtifactPaths {
    pub artifact_digest: String,
    pub artifact_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub qualification_path: PathBuf,
    pub source_checkout_dir: PathBuf,
    pub current_pointer: PathBuf,
}

/// Facts that qualify one immutable live-corpus artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCorpusArtifactIdentity<'a> {
    pub lock_digest: &'a str,
    pub resource_id: &'a str,
    pub provider_id: &'a str,
    pub language_id: &'a str,
    pub builder_id: &'a str,
    pub revision: &'a str,
    pub git_tree: &'a str,
    pub source_merkle_root: &'a str,
}

/// Immutable identity manifest stored in a live-corpus artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCorpusArtifactManifest {
    pub schema_id: String,
    pub schema_version: String,
    pub artifact_digest: String,
    pub lock_digest: String,
    pub resource_id: String,
    pub provider_id: String,
    pub language_id: String,
    pub builder_id: String,
    pub git: LiveCorpusArtifactGitIdentity,
    pub source_merkle_root: String,
}

/// Git identity embedded in the immutable artifact manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCorpusArtifactGitIdentity {
    pub remote: String,
    pub canonical_remote_identity: String,
    pub remote_digest: String,
    pub revision: String,
    pub tree: String,
}

/// Git facts read from an explicitly materialized immutable checkout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCorpusGitCheckoutQualification {
    pub canonical_remote_identity: String,
    pub head_revision: String,
    pub git_tree: String,
    pub checkout_identity_digest: String,
}

/// Provider-owned language-extension evidence read from a clean Git index.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCorpusLanguageExtensionEvidence {
    pub authority: String,
    pub candidate_set_authority: String,
    pub source_extensions: Vec<String>,
    pub matching_file_count: usize,
    pub candidate_language_file_count: usize,
}

/// Derive the authoritative repository directory and readable GHQ-style alias.
///
/// The alias is diagnostic only. `repository_dir` is the collision-resistant
/// authority and depends exclusively on the normalized remote identity.
pub fn live_corpus_git_repository_paths(
    state_home: &Path,
    remote: &str,
) -> Result<LiveCorpusGitRepositoryPaths, String> {
    let canonical_remote_identity = RemoteUrl(remote.to_owned())
        .canonical_identity()
        .ok_or_else(|| format!("live corpus Git remote is not canonicalizable: {remote}"))?;
    let remote_digest = blake3::hash(canonical_remote_identity.as_bytes())
        .to_hex()
        .to_string();
    let repository_dir = state_home
        .join("git")
        .join("repo")
        .join(BLAKE3_256)
        .join(&remote_digest);
    let ghq_alias_path = canonical_remote_identity
        .split('/')
        .fold(state_home.join("git").join("by-remote"), |path, segment| {
            path.join(encode_path_segment(segment))
        });

    Ok(LiveCorpusGitRepositoryPaths {
        canonical_remote_identity,
        remote_digest,
        repository_dir,
        ghq_alias_path,
    })
}

/// Explicitly acquire and publish one pinned live-corpus checkout through
/// `gix`. Search and benchmark commands never call this implicitly.
pub fn sync_live_corpus_git_checkout(
    state_home: &Path,
    remote: &str,
    revision: &str,
) -> Result<LiveCorpusGitCheckoutSync, String> {
    validate_hex("revision", revision, &[40])?;
    let paths = live_corpus_git_repository_paths(state_home, remote)?;
    let checkout_dir = paths.repository_dir.join("checkouts").join(revision);
    if checkout_dir.exists() {
        qualify_reusable_checkout(&checkout_dir, remote, revision)?;
        return Ok(LiveCorpusGitCheckoutSync {
            checkout_dir,
            reused: true,
        });
    }

    let checkouts_dir = paths.repository_dir.join("checkouts");
    fs::create_dir_all(&checkouts_dir).map_err(|error| {
        format!(
            "failed to create live-corpus checkout directory {}: {error}",
            checkouts_dir.display()
        )
    })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("failed to derive live-corpus sync nonce: {error}"))?
        .as_nanos();
    let temporary_checkout =
        checkouts_dir.join(format!(".sync-{revision}-{}-{nonce}", std::process::id()));

    let synchronize = || -> Result<(), String> {
        let mut preparation = gix::prepare_clone(remote, &temporary_checkout)
            .map_err(|error| format!("failed to prepare live-corpus gix clone: {error}"))?;
        let should_interrupt = AtomicBool::new(false);
        let (repository, _) = preparation
            .fetch_only(gix::progress::Discard, &should_interrupt)
            .map_err(|error| format!("failed to fetch live-corpus through gix: {error}"))?;
        checkout_pinned_revision(&repository, revision, &should_interrupt)?;
        drop(repository);
        qualify_reusable_checkout(&temporary_checkout, remote, revision)?;
        fs::rename(&temporary_checkout, &checkout_dir).map_err(|error| {
            format!(
                "failed to atomically publish live-corpus checkout {}: {error}",
                checkout_dir.display()
            )
        })
    };

    if let Err(error) = synchronize() {
        let _ = fs::remove_dir_all(&temporary_checkout);
        if checkout_dir.exists()
            && qualify_reusable_checkout(&checkout_dir, remote, revision).is_ok()
        {
            return Ok(LiveCorpusGitCheckoutSync {
                checkout_dir,
                reused: true,
            });
        }
        return Err(error);
    }

    Ok(LiveCorpusGitCheckoutSync {
        checkout_dir,
        reused: false,
    })
}

fn checkout_pinned_revision(
    repository: &gix::Repository,
    revision: &str,
    should_interrupt: &AtomicBool,
) -> Result<(), String> {
    use gix::refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit},
    };

    let object_id = gix::ObjectId::from_hex(revision.as_bytes())
        .map_err(|error| format!("invalid live-corpus revision object id: {error}"))?;
    let commit = repository
        .find_commit(object_id)
        .map_err(|error| format!("locked live-corpus revision was not fetched: {error}"))?;
    let tree_id = commit
        .tree_id()
        .map_err(|error| format!("failed to resolve locked live-corpus tree: {error}"))?
        .detach();
    let mut index = repository
        .index_from_tree(&tree_id)
        .map_err(|error| format!("failed to create live-corpus index from locked tree: {error}"))?;
    let workdir = repository
        .workdir()
        .ok_or_else(|| "gix live-corpus clone has no worktree".to_string())?;
    let mut options = repository
        .checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)
        .map_err(|error| format!("failed to prepare live-corpus checkout options: {error}"))?;
    options.destination_is_initially_empty = true;
    gix::worktree::state::checkout(
        &mut index,
        workdir,
        repository
            .objects
            .clone()
            .into_arc()
            .map_err(|error| format!("failed to prepare live-corpus object store: {error}"))?,
        &gix::progress::Discard,
        &gix::progress::Discard,
        should_interrupt,
        options,
    )
    .map_err(|error| format!("failed to check out locked live-corpus tree: {error}"))?;
    index
        .write(Default::default())
        .map_err(|error| format!("failed to publish live-corpus Git index: {error}"))?;
    repository
        .edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange::default(),
                expected: PreviousValue::Any,
                new: Target::Object(object_id),
            },
            name: "HEAD".try_into().expect("HEAD is a valid reference"),
            deref: false,
        })
        .map_err(|error| format!("failed to detach live-corpus HEAD: {error}"))?;
    Ok(())
}

fn qualify_reusable_checkout(
    checkout_dir: &Path,
    remote: &str,
    revision: &str,
) -> Result<(), String> {
    qualify_live_corpus_git_checkout(checkout_dir, remote, revision)?;
    if !live_corpus_git_checkout_is_clean(checkout_dir)? {
        return Err(format!(
            "live corpus checkout is dirty: {}",
            checkout_dir.display()
        ));
    }
    Ok(())
}

/// Digest the exact validated lock bytes bound into artifact identity.
#[must_use]
pub fn live_corpus_lock_digest(lock_bytes: &[u8]) -> String {
    blake3::hash(lock_bytes).to_hex().to_string()
}

/// Read checkout identity through `gix` without invoking a Git subprocess.
pub fn qualify_live_corpus_git_checkout(
    source_root: &Path,
    locked_remote: &str,
    locked_revision: &str,
) -> Result<LiveCorpusGitCheckoutQualification, String> {
    validate_hex("revision", locked_revision, &[40])?;
    let expected = RemoteUrl(locked_remote.to_string())
        .canonical_identity()
        .ok_or_else(|| "live corpus locked remote is not canonicalizable".to_string())?;
    let repository = gix::discover(source_root)
        .map_err(|error| format!("failed to discover live corpus Git checkout: {error}"))?;
    let actual_remote = crate::git::canonical_remote_url_from_repository(&repository)
        .and_then(|remote| RemoteUrl(remote).canonical_identity())
        .ok_or_else(|| "live corpus checkout has no canonical Git remote".to_string())?;
    if actual_remote != expected {
        return Err(format!(
            "live corpus checkout remote mismatch: expected={expected} actual={actual_remote}"
        ));
    }
    let commit = repository
        .head_commit()
        .map_err(|error| format!("failed to resolve live corpus HEAD commit: {error}"))?;
    let head_revision = commit.id.to_string();
    if head_revision != locked_revision {
        return Err(format!(
            "live corpus checkout revision mismatch: expected={locked_revision} actual={head_revision}"
        ));
    }
    let git_tree = commit
        .tree_id()
        .map_err(|error| format!("failed to resolve live corpus Git tree: {error}"))?
        .to_string();
    let checkout_identity_digest =
        digest_fields(&["live-corpus-git-tree", &expected, &head_revision, &git_tree]);

    Ok(LiveCorpusGitCheckoutQualification {
        canonical_remote_identity: expected,
        head_revision,
        git_tree,
        checkout_identity_digest,
    })
}

/// Verify that the explicitly materialized checkout has no tracked, untracked,
/// staged, or worktree changes.
pub fn live_corpus_git_checkout_is_clean(source_root: &Path) -> Result<bool, String> {
    let repository = gix::discover(source_root)
        .map_err(|error| format!("failed to discover live corpus Git checkout: {error}"))?;
    let status = repository
        .status(gix::progress::Discard)
        .map_err(|error| format!("failed to initialize live corpus Git status: {error}"))?;
    let mut changes = status
        .into_iter(std::iter::empty::<gix::bstr::BString>())
        .map_err(|error| format!("failed to inspect live corpus Git status: {error}"))?;
    Ok(changes
        .next()
        .transpose()
        .map_err(|error| format!("failed to inspect live corpus Git status entry: {error}"))?
        .is_none())
}

/// Count target-language files against the union of activated provider
/// extension triggers. Binary attachments and unknown suffixes are excluded
/// from the ratio denominator.
pub fn qualify_live_corpus_language_extensions(
    source_root: &Path,
    source_extensions: &[String],
    provider_registry_extensions: &[String],
) -> Result<LiveCorpusLanguageExtensionEvidence, String> {
    let target = normalize_extension_set("sourceExtensions", source_extensions)?;
    let candidates =
        normalize_extension_set("providerRegistryExtensions", provider_registry_extensions)?;
    if !target.is_subset(&candidates) {
        return Err(
            "live corpus source extensions are absent from the provider registry extension index"
                .to_string(),
        );
    }
    let repository = gix::discover(source_root)
        .map_err(|error| format!("failed to discover live corpus Git checkout: {error}"))?;
    let index = repository
        .index()
        .map_err(|error| format!("failed to read live corpus Git index: {error}"))?;
    let paths = index
        .entries()
        .iter()
        .map(|entry| String::from_utf8_lossy(entry.path(&index).as_ref()).into_owned());
    language_extension_evidence_from_paths(paths, target, candidates)
}

fn language_extension_evidence_from_paths(
    paths: impl IntoIterator<Item = String>,
    target: BTreeSet<String>,
    candidates: BTreeSet<String>,
) -> Result<LiveCorpusLanguageExtensionEvidence, String> {
    let mut matching_file_count = 0;
    let mut candidate_language_file_count = 0;
    for path in paths {
        let Some(extension) = Path::new(&path)
            .extension()
            .map(|extension| format!(".{}", extension.to_string_lossy().to_ascii_lowercase()))
        else {
            continue;
        };
        if candidates.contains(&extension) {
            candidate_language_file_count += 1;
        }
        if target.contains(&extension) {
            matching_file_count += 1;
        }
    }
    if matching_file_count == 0 || candidate_language_file_count < matching_file_count {
        return Err(
            "live corpus Git index has no valid target-language extension coverage".to_string(),
        );
    }
    Ok(LiveCorpusLanguageExtensionEvidence {
        authority: "provider-project-resolution".to_string(),
        candidate_set_authority: "provider-registry-extension-index".to_string(),
        source_extensions: target.into_iter().collect(),
        matching_file_count,
        candidate_language_file_count,
    })
}

fn normalize_extension_set(field: &str, extensions: &[String]) -> Result<BTreeSet<String>, String> {
    let normalized = extensions
        .iter()
        .map(|extension| {
            let extension = extension.trim();
            if extension.starts_with('.') {
                extension.to_ascii_lowercase()
            } else {
                format!(".{}", extension.to_ascii_lowercase())
            }
        })
        .collect::<BTreeSet<_>>();
    if normalized.is_empty()
        || normalized.iter().any(|extension| {
            extension.len() < 2
                || !extension[1..].bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-')
                })
        })
    {
        return Err(format!("live corpus {field} is invalid"));
    }
    Ok(normalized)
}

/// Derive the immutable artifact directory for a qualified locked checkout.
pub fn live_corpus_artifact_paths(
    state_home: &Path,
    repository: &LiveCorpusGitRepositoryPaths,
    identity: &LiveCorpusArtifactIdentity<'_>,
) -> Result<LiveCorpusArtifactPaths, String> {
    validate_hex("lockDigest", identity.lock_digest, &[64])?;
    validate_identifier("resourceId", identity.resource_id)?;
    validate_identifier("providerId", identity.provider_id)?;
    validate_identifier("languageId", identity.language_id)?;
    validate_identifier("builderId", identity.builder_id)?;
    validate_hex("revision", identity.revision, &[40])?;
    validate_hex("gitTree", identity.git_tree, &[40, 64])?;
    validate_hex("sourceMerkleRoot", identity.source_merkle_root, &[64])?;

    let artifact_digest = digest_fields(&[
        "live-corpus-artifact",
        identity.lock_digest,
        &repository.canonical_remote_identity,
        identity.resource_id,
        identity.provider_id,
        identity.language_id,
        identity.builder_id,
        identity.revision,
        identity.git_tree,
        identity.source_merkle_root,
    ]);
    let artifact_root = state_home.join("artifacts").join("live-corpus");
    let artifact_dir = artifact_root.join(BLAKE3_256).join(&artifact_digest);

    Ok(LiveCorpusArtifactPaths {
        manifest_path: artifact_dir.join("manifest.json"),
        qualification_path: artifact_dir.join("qualification.json"),
        source_checkout_dir: repository
            .repository_dir
            .join("checkouts")
            .join(identity.revision),
        current_pointer: artifact_root
            .join("by-resource")
            .join(identity.resource_id)
            .join("current"),
        artifact_digest,
        artifact_dir,
    })
}

/// Build the immutable typed manifest for an already-derived artifact path.
pub fn live_corpus_artifact_manifest(
    remote: &str,
    repository: &LiveCorpusGitRepositoryPaths,
    identity: &LiveCorpusArtifactIdentity<'_>,
    paths: &LiveCorpusArtifactPaths,
) -> Result<LiveCorpusArtifactManifest, String> {
    let expected_repository = live_corpus_git_repository_paths(Path::new(""), remote)?;
    if expected_repository.canonical_remote_identity != repository.canonical_remote_identity
        || expected_repository.remote_digest != repository.remote_digest
    {
        return Err("live corpus manifest remote does not match repository identity".to_string());
    }
    let expected_paths = live_corpus_artifact_paths(Path::new(""), &expected_repository, identity)?;
    if expected_paths.artifact_digest != paths.artifact_digest {
        return Err("live corpus manifest identity does not match artifact digest".to_string());
    }

    Ok(LiveCorpusArtifactManifest {
        schema_id: LIVE_CORPUS_ARTIFACT_SCHEMA_ID.to_string(),
        schema_version: LIVE_CORPUS_ARTIFACT_SCHEMA_VERSION.to_string(),
        artifact_digest: paths.artifact_digest.clone(),
        lock_digest: identity.lock_digest.to_string(),
        resource_id: identity.resource_id.to_string(),
        provider_id: identity.provider_id.to_string(),
        language_id: identity.language_id.to_string(),
        builder_id: identity.builder_id.to_string(),
        git: LiveCorpusArtifactGitIdentity {
            remote: remote.to_string(),
            canonical_remote_identity: repository.canonical_remote_identity.clone(),
            remote_digest: repository.remote_digest.clone(),
            revision: identity.revision.to_string(),
            tree: identity.git_tree.to_string(),
        },
        source_merkle_root: identity.source_merkle_root.to_string(),
    })
}

fn digest_fields(fields: &[&str]) -> String {
    let mut hasher = blake3::Hasher::new();
    for field in fields {
        hasher.update(&(field.len() as u64).to_le_bytes());
        hasher.update(field.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn validate_identifier(name: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "live corpus {name} is not a path-safe identifier: {value}"
        ))
    }
}

fn validate_hex(name: &str, value: &str, lengths: &[usize]) -> Result<(), String> {
    if lengths.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!(
            "live corpus {name} is not canonical lowercase hex: {value}"
        ))
    }
}

fn encode_path_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02x}"));
        }
    }
    encoded
}

#[cfg(test)]
#[path = "../tests/unit/live_corpus.rs"]
mod tests;
