// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! PATH-visible `asp` binary installation helpers.

use agent_semantic_artifacts::runtime_artifact_member_is_digest_addressed;

#[cfg(test)]
fn protocol_binary_artifact_path_digest(path: &Path) -> Option<String> {
    let launcher_target = std::fs::read_link(path).ok()?;
    let launcher_target = if launcher_target.is_absolute() {
        launcher_target
    } else {
        path.parent()?.join(launcher_target)
    };
    let active_slot = launcher_target.parent()?;
    let generation_target = std::fs::read_link(active_slot).ok()?;
    let generation_target = if generation_target.is_absolute() {
        generation_target
    } else {
        active_slot.parent()?.join(generation_target)
    };
    let generation = generation_target.file_name()?.to_str()?;
    (generation.len() == 64
        && generation
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then(|| generation.to_owned())
}

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub(crate) const SEMANTIC_AGENT_PROTOCOL_BIN: &str = "asp";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeBinaryIdentityV1 {
    name: std::ffi::OsString,
}

impl RuntimeBinaryIdentityV1 {
    pub(crate) fn asp_bootstrap() -> Self {
        Self {
            name: SEMANTIC_AGENT_PROTOCOL_BIN.into(),
        }
    }

    pub(crate) fn from_registered_provider(binary: &str) -> Result<Self, String> {
        let name = Path::new(binary);
        if binary.is_empty()
            || name.components().count() != 1
            || name.file_name().and_then(|value| value.to_str()) != Some(binary)
        {
            return Err(format!(
                "ProviderRegistry binary must be one executable name, got `{binary}`"
            ));
        }
        Ok(Self {
            name: binary.into(),
        })
    }

    fn name(&self) -> &std::ffi::OsStr {
        &self.name
    }
}
#[cfg(test)]
static PROTOCOL_BINARY_PUBLISH_SEQUENCE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
fn next_protocol_binary_publish_sequence() -> u64 {
    PROTOCOL_BINARY_PUBLISH_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
const SEMANTIC_AGENT_BIN_DIR_ENV: &str = "SEMANTIC_AGENT_BIN_DIR";
#[derive(Debug)]
pub(crate) struct ProtocolBinaryInstall {
    pub(crate) path: PathBuf,
    pub(crate) status: &'static str,
    pub(crate) artifact_digest: String,
    pub(crate) bundle_digest: Option<String>,
    pub(crate) lock_acquisition_count: u8,
    pub(crate) quiescence_operation: Option<String>,
    pub(crate) quiescence_lease_nonce: Option<String>,
    pub(crate) lease_producer_process_id: Option<u32>,
    pub(crate) lease_consumer_process_id: Option<u32>,
}

impl ProtocolBinaryInstall {
    fn validate_artifact_transaction_receipt(&self) -> Result<(), String> {
        if self.status == "current" {
            return Ok(());
        }
        let expected_operation = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("publish:{name}"))
            .ok_or_else(|| {
                "reasonKind=runtime-artifact-publication-receipt-incomplete missing binary name"
                    .to_owned()
            })?;
        self.validate_artifact_transaction_receipt_for_operation(&expected_operation)
    }

    fn validate_artifact_transaction_receipt_for_operation(
        &self,
        expected_operation: &str,
    ) -> Result<(), String> {
        if self.status == "current" {
            return Ok(());
        }
        if self.lock_acquisition_count != 1
            || self.quiescence_operation.as_deref() != Some(expected_operation)
            || self
                .quiescence_lease_nonce
                .as_deref()
                .is_none_or(|nonce| nonce.is_empty())
            || self
                .lease_producer_process_id
                .is_none_or(|process_id| process_id == 0)
            || self.lease_consumer_process_id != Some(std::process::id())
        {
            return Err(format!(
                "reasonKind=runtime-artifact-publication-receipt-incomplete operation={} lockAcquisitionCount={} producerProcessId={:?} consumerProcessId={:?}",
                self.quiescence_operation.as_deref().unwrap_or("missing"),
                self.lock_acquisition_count,
                self.lease_producer_process_id,
                self.lease_consumer_process_id
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtocolBinaryInstallPlan {
    current_exe: PathBuf,
    explicit_candidate_source: Option<PathBuf>,
    target: PathBuf,
    artifact_root: PathBuf,
    binary_identity: RuntimeBinaryIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProtocolBinaryInstallEntries {
    path_visible: PathBuf,
    runtime_alias: PathBuf,
}

impl ProtocolBinaryInstallPlan {
    pub(crate) fn candidate_source(&self) -> &std::path::Path {
        self.explicit_candidate_source
            .as_deref()
            .unwrap_or(&self.current_exe)
    }

    pub(crate) fn install_source_kind(&self) -> &'static str {
        if self.explicit_candidate_source.is_some() {
            "explicit-target"
        } else {
            "current-executable"
        }
    }

    pub(crate) fn current_exe(&self) -> &Path {
        &self.current_exe
    }
}

/// Runtime-owned transaction guard for artifact mutation and its coupled
/// Protocol receipts. Protocol carries this authority; it does not own a
/// second reconciliation lock.
#[cfg(test)]
#[path = "../../tests/unit/protocol_binary_reconciliation_lock.rs"]
mod protocol_binary_reconciliation_lock_tests;

impl ProtocolBinaryInstallPlan {
    pub(crate) fn capture(artifact_root: PathBuf) -> Result<Self, String> {
        let current_exe = env::current_exe()
            .map_err(|error| format!("failed to resolve current protocol binary: {error}"))?;
        let current_name = current_exe
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if current_name.trim_end_matches(".exe") != SEMANTIC_AGENT_PROTOCOL_BIN {
            return Err(format!(
                "semantic hook setup must run through `{SEMANTIC_AGENT_PROTOCOL_BIN}` so generated hooks can resolve the same binary on PATH"
            ));
        }

        let explicit_bin_dir = env::var_os(SEMANTIC_AGENT_BIN_DIR_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        if let Some(bin_dir) = explicit_bin_dir.as_ref() {
            fs::create_dir_all(bin_dir)
                .map_err(|error| format!("failed to create {}: {error}", bin_dir.display()))?;
        }
        require_configured_protocol_bin_dir_on_path()?;
        let home = match explicit_bin_dir.as_ref() {
            Some(_) => PathBuf::new(),
            None => env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| {
                    "protocol binary installation requires HOME or SEMANTIC_AGENT_BIN_DIR"
                        .to_owned()
                })?,
        };
        let entries = resolve_protocol_binary_install_entries(
            explicit_bin_dir.as_deref(),
            &artifact_root,
            &home,
        )?;
        if same_protocol_binary_entry(&entries.path_visible, &entries.runtime_alias) {
            return Err(
                "reasonKind=runtime-protocol-binary-direction-invalid PATH-visible entry and Runtime compatibility alias must be distinct"
                    .to_owned(),
            );
        }
        Ok(Self {
            current_exe,
            explicit_candidate_source: None,
            target: entries.path_visible,
            artifact_root,
            binary_identity: RuntimeBinaryIdentityV1::asp_bootstrap(),
        })
    }
}

#[cfg(test)]
pub(crate) async fn ensure_protocol_binary_installed(
    plan: &ProtocolBinaryInstallPlan,
) -> Result<ProtocolBinaryInstall, String> {
    install_protocol_binary_target(
        plan.candidate_source(),
        &plan.target,
        &plan.artifact_root,
        &plan.binary_identity,
    )
    .await
}

pub(crate) async fn ensure_protocol_binary_bundle_installed_transaction(
    plan: &ProtocolBinaryInstallPlan,
    hook_source: &Path,
) -> Result<ProtocolBinaryInstall, String> {
    let members = [
        agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
            name: "asp-hook",
            source: hook_source,
        },
    ];
    ensure_protocol_binary_bundle_members_installed_transaction(plan, &members).await
}

pub(crate) async fn ensure_protocol_binary_bundle_members_installed_transaction(
    plan: &ProtocolBinaryInstallPlan,
    members: &[agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource<'_>],
) -> Result<ProtocolBinaryInstall, String> {
    ensure_protocol_binary_bundle_members_installed_transaction_with_binding(
        plan, members, None, None,
    )
    .await
}

pub(crate) async fn ensure_protocol_binary_bound_bundle_members_installed_transaction(
    plan: &ProtocolBinaryInstallPlan,
    members: &[agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource<'_>],
    execution_binding: &agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding,
    predecessor_bundle: Option<(
        &Path,
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    )>,
) -> Result<ProtocolBinaryInstall, String> {
    ensure_protocol_binary_bundle_members_installed_transaction_with_binding(
        plan,
        members,
        Some(execution_binding),
        predecessor_bundle,
    )
    .await
}

async fn ensure_protocol_binary_bundle_members_installed_transaction_with_binding(
    plan: &ProtocolBinaryInstallPlan,
    members: &[agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource<'_>],
    execution_binding: Option<
        &agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding,
    >,
    predecessor_bundle: Option<(
        &Path,
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    )>,
) -> Result<ProtocolBinaryInstall, String> {
    let runtime_root = plan.artifact_root.parent().ok_or_else(|| {
        format!(
            "runtime artifact root has no Runtime parent: {}",
            plan.artifact_root.display()
        )
    })?;
    let state_home = runtime_root.parent().ok_or_else(|| {
        format!(
            "Runtime root has no State Home parent: {}",
            runtime_root.display()
        )
    })?;
    let artifact_mode =
        if agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_developer_root(
            state_home,
        )?
        .is_some()
        {
            "dev"
        } else {
            "release"
        };
    let receipt = match execution_binding {
        Some(binding) => {
            match predecessor_bundle {
                Some((predecessor_path, predecessor_digest)) => {
                    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bound_bundle_members_for_binary_replacement(
                        state_home,
                        plan.candidate_source(),
                        &plan.target,
                        artifact_mode,
                        members,
                        binding,
                        predecessor_path,
                        predecessor_digest,
                    )
                    .await?
                }
                None => {
                    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bound_bundle_members(
                        state_home,
                        plan.candidate_source(),
                        &plan.target,
                        artifact_mode,
                        members,
                        binding,
                    )
                    .await?
                }
            }
        }
        None => {
            agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bundle_successor_from_active(
                state_home,
                plan.candidate_source(),
                &plan.target,
                artifact_mode,
                members,
            )
            .await?
        }
    };
    let install = ProtocolBinaryInstall {
        path: receipt.path,
        status: receipt.status,
        artifact_digest: receipt.artifact_digest.to_string(),
        bundle_digest: Some(receipt.bundle_digest.to_string()),
        lock_acquisition_count: receipt.lock_acquisition_count,
        quiescence_operation: Some(receipt.quiescence_operation),
        quiescence_lease_nonce: Some(receipt.quiescence_lease_nonce),
        lease_producer_process_id: Some(receipt.lease_producer_process_id),
        lease_consumer_process_id: Some(receipt.lease_consumer_process_id),
    };
    install.validate_artifact_transaction_receipt_for_operation("publish:asp")?;
    Ok(install)
}

fn protocol_binary_symlink_chain_loops(candidate: &Path) -> bool {
    let mut current = candidate.to_path_buf();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        if !seen.insert(current.clone()) {
            return true;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            return false;
        };
        if !metadata.file_type().is_symlink() {
            return false;
        }
        let Ok(target) = std::fs::read_link(&current) else {
            return false;
        };
        current = if target.is_absolute() {
            target
        } else {
            current
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(target)
        };
    }
    true
}

fn resolve_protocol_binary_install_entries(
    explicit_bin_dir: Option<&Path>,
    artifact_root: &Path,
    user_home: &Path,
) -> Result<ProtocolBinaryInstallEntries, String> {
    let path_visible = explicit_bin_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| user_home.join(".local/bin"))
        .join(SEMANTIC_AGENT_PROTOCOL_BIN);
    Ok(ProtocolBinaryInstallEntries {
        path_visible,
        runtime_alias: runtime_protocol_binary_alias(artifact_root)?,
    })
}

fn runtime_protocol_binary_alias(artifact_root: &Path) -> Result<PathBuf, String> {
    if artifact_root.file_name().and_then(|name| name.to_str()) != Some("artifacts") {
        return Err(format!(
            "protocol artifact root must end in `artifacts`: {}",
            artifact_root.display()
        ));
    }
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "protocol artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    Ok(runtime_root.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN))
}

pub(crate) async fn install_protocol_binary_target(
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    binary_identity: &RuntimeBinaryIdentityV1,
) -> Result<ProtocolBinaryInstall, String> {
    if binary_identity.name() != std::ffi::OsStr::new(SEMANTIC_AGENT_PROTOCOL_BIN) {
        return install_release_provider_member_target(
            source,
            target,
            artifact_root,
            binary_identity,
        )
        .await;
    }
    install_protocol_binary_target_transaction(
        source,
        target,
        artifact_root,
        binary_identity,
        None,
        &[],
    )
    .await
}

async fn install_release_provider_member_target(
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    binary_identity: &RuntimeBinaryIdentityV1,
) -> Result<ProtocolBinaryInstall, String> {
    let binary_name = binary_identity.name();
    if target.file_name() != Some(binary_name) {
        return Err(format!(
            "provider binary target {} does not match declared binary identity `{}`",
            target.display(),
            binary_name.to_string_lossy()
        ));
    }
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "runtime artifact root has no Runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let state_home = runtime_root.parent().ok_or_else(|| {
        format!(
            "Runtime root has no State Home parent: {}",
            runtime_root.display()
        )
    })?;
    let expected_target = runtime_root.join("bin").join(binary_name);
    if !same_protocol_binary_entry(target, &expected_target) {
        return Err(format!(
            "provider binary target must use the Runtime stable launcher: expected={} actual={}",
            expected_target.display(),
            target.display()
        ));
    }
    let member_name = binary_name
        .to_str()
        .ok_or_else(|| "provider binary identity must be UTF-8".to_owned())?;
    let member_digest =
        agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(source)
            .await?;
    let publication = agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bound_provider_member_from_active(
        state_home,
        member_name,
        source,
        "release",
    )
    .await?;
    let install = ProtocolBinaryInstall {
        path: target.to_path_buf(),
        status: publication.status,
        artifact_digest: member_digest.to_string(),
        bundle_digest: Some(publication.bundle_digest.to_string()),
        lock_acquisition_count: publication.lock_acquisition_count,
        quiescence_operation: Some(publication.quiescence_operation),
        quiescence_lease_nonce: Some(publication.quiescence_lease_nonce),
        lease_producer_process_id: Some(publication.lease_producer_process_id),
        lease_consumer_process_id: Some(publication.lease_consumer_process_id),
    };
    install.validate_artifact_transaction_receipt_for_operation("publish:asp")?;
    Ok(install)
}

pub(crate) async fn install_qualified_provider_staging_target(
    source: &Path,
    target: &Path,
    binary_identity: &RuntimeBinaryIdentityV1,
    checkout_root: PathBuf,
) -> Result<ProtocolBinaryInstall, String> {
    let authority = agent_semantic_artifacts::runtime_artifact_catalog::QualifiedRuntimeArtifactSource::develop_state_home_staging(checkout_root)?;
    let binary_name = binary_identity.name();
    if target.file_name() != Some(binary_name) {
        return Err(format!(
            "provider binary target {} does not match declared binary identity `{}`",
            target.display(),
            binary_name.to_string_lossy()
        ));
    }
    let state_home = qualified_provider_state_home(target, binary_name)?;
    let artifact_kind = binary_name.to_string_lossy().into_owned();
    authority.validate_source(&state_home, source, &artifact_kind)?;
    let member_digest =
        agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(source)
            .await?;
    let publication = agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bound_provider_member_from_active(
        &state_home,
        &artifact_kind,
        source,
        "dev",
    )
    .await?;
    Ok(ProtocolBinaryInstall {
        path: target.to_path_buf(),
        status: publication.status,
        artifact_digest: member_digest.to_string(),
        bundle_digest: Some(publication.bundle_digest.to_string()),
        lock_acquisition_count: publication.lock_acquisition_count,
        quiescence_operation: Some(publication.quiescence_operation),
        quiescence_lease_nonce: Some(publication.quiescence_lease_nonce),
        lease_producer_process_id: Some(publication.lease_producer_process_id),
        lease_consumer_process_id: Some(publication.lease_consumer_process_id),
    })
}

fn qualified_provider_state_home(
    target: &Path,
    binary_name: &std::ffi::OsStr,
) -> Result<PathBuf, String> {
    let runtime_bin = target.parent().ok_or_else(|| {
        format!(
            "provider binary target has no Runtime bin parent: {}",
            target.display()
        )
    })?;
    let runtime_root = runtime_bin.parent().ok_or_else(|| {
        format!(
            "Runtime bin has no Runtime parent: {}",
            runtime_bin.display()
        )
    })?;
    let state_home = runtime_root.parent().ok_or_else(|| {
        format!(
            "Runtime root has no state-home parent: {}",
            runtime_root.display()
        )
    })?;
    let expected_target = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .bin()
        .join(binary_name);
    if !same_protocol_binary_entry(target, &expected_target) {
        return Err(format!(
            "provider binary target must use the Runtime stable launcher: expected={} actual={}",
            expected_target.display(),
            target.display()
        ));
    }
    Ok(state_home.to_path_buf())
}

async fn install_protocol_binary_target_transaction(
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    binary_identity: &RuntimeBinaryIdentityV1,
    qualified_source: Option<
        agent_semantic_artifacts::runtime_artifact_catalog::QualifiedRuntimeArtifactSource,
    >,
    bundle_members: &[agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource<'_>],
) -> Result<ProtocolBinaryInstall, String> {
    let binary_name = binary_identity.name();
    if target.file_name() != Some(binary_name) {
        return Err(format!(
            "runtime binary target {} does not match declared binary identity `{}`",
            target.display(),
            binary_name.to_string_lossy()
        ));
    }
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "runtime artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let state_home = runtime_root.parent().ok_or_else(|| {
        format!(
            "runtime root has no state-home parent: {}",
            runtime_root.display()
        )
    })?;
    let forbidden_reverse_target = runtime_root.join("bin").join(binary_name);
    if same_protocol_binary_entry(target, &forbidden_reverse_target) {
        return Err(format!(
            "reasonKind=runtime-protocol-binary-direction-invalid PATH-visible target must not be the Runtime compatibility alias: {}",
            target.display(),
        ));
    }
    let artifact_kind = binary_name.to_string_lossy().into_owned();
    let mut developer_source = qualified_source.is_some();
    if let Some(authority) = qualified_source.as_ref() {
        authority.validate_source(state_home, source, &artifact_kind)?;
    }
    let developer_root =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_developer_root(
            state_home,
        )?;
    if let Some(developer_root) = developer_root.as_ref() {
        let source_identity = std::fs::canonicalize(source).map_err(|error| {
            format!(
                "failed to resolve Developer Runtime binary {}: {error}",
                source.display()
            )
        })?;
        if source_identity.starts_with(developer_root) {
            developer_source = true;
        }
    } else {
        validate_protocol_entry_for_repair(target, artifact_root)?;
    }
    let artifact_mode = if developer_source { "dev" } else { "release" };
    let receipt = if bundle_members.is_empty() {
        agent_semantic_runtime_server::resident_install::install_resident_runtime(
            state_home,
            source,
            target,
            artifact_mode,
            qualified_source,
        )
        .await?
    } else {
        agent_semantic_runtime_server::resident_install::install_resident_runtime_bundle_members(
            state_home,
            source,
            target,
            bundle_members,
            artifact_mode,
            qualified_source,
        )
        .await?
    };
    let install = ProtocolBinaryInstall {
        path: receipt.path,
        status: receipt.status,
        artifact_digest: receipt.artifact_digest.to_string(),
        bundle_digest: Some(receipt.bundle_digest.to_string()),
        lock_acquisition_count: receipt.lock_acquisition_count,
        quiescence_operation: Some(receipt.quiescence_operation),
        quiescence_lease_nonce: Some(receipt.quiescence_lease_nonce),
        lease_producer_process_id: Some(receipt.lease_producer_process_id),
        lease_consumer_process_id: Some(receipt.lease_consumer_process_id),
    };
    install.validate_artifact_transaction_receipt()?;
    Ok(install)
}

fn resolve_protocol_binary_artifact_entry(
    entry: &Path,
    artifact_root: &Path,
) -> Result<PathBuf, String> {
    if protocol_binary_symlink_chain_loops(entry) {
        return Err(format!(
            "protocol binary symlink chain loops at {}",
            entry.display()
        ));
    }
    let identity = fs::canonicalize(entry).map_err(|error| {
        format!(
            "failed to resolve protocol binary entry {}: {error}",
            entry.display()
        )
    })?;
    if !runtime_artifact_member_is_digest_addressed(&identity, artifact_root)? {
        return Err(format!(
            "protocol binary entry {} escapes immutable artifact root {}",
            entry.display(),
            artifact_root.display()
        ));
    }
    Ok(identity)
}

fn validate_protocol_entry_for_repair(entry: &Path, artifact_root: &Path) -> Result<(), String> {
    if protocol_binary_symlink_chain_loops(entry) {
        return Err(format!(
            "protocol binary symlink chain loops at {}",
            entry.display()
        ));
    }
    let Ok(metadata) = fs::symlink_metadata(entry) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        match fs::metadata(entry) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(format!(
                    "failed to inspect protocol binary entry {}: {error}",
                    entry.display()
                ));
            }
        }
        resolve_protocol_binary_artifact_entry(entry, artifact_root)?;
        return Ok(());
    }
    if metadata.is_file() {
        return Ok(());
    }
    Err(format!(
        "refusing to replace non-file protocol binary entry {}",
        entry.display()
    ))
}

#[cfg(all(test, unix))]
#[path = "../../tests/unit/protocol_binary_latest_publication.rs"]
mod protocol_binary_latest_publication_tests;

fn require_path_contains_dir(dir: &Path) -> Result<(), String> {
    if path_dirs().iter().any(|path_dir| same_dir(path_dir, dir)) {
        Ok(())
    } else {
        Err(format!(
            "{SEMANTIC_AGENT_BIN_DIR_ENV}={} is not present in PATH; generated hooks use bare `{SEMANTIC_AGENT_PROTOCOL_BIN}`",
            dir.display()
        ))
    }
}

pub(crate) fn require_configured_protocol_bin_dir_on_path() -> Result<(), String> {
    if let Some(bin_dir) = env::var_os(SEMANTIC_AGENT_BIN_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        require_path_contains_dir(&bin_dir)?;
    }
    Ok(())
}

fn path_dirs() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

fn same_dir(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn same_protocol_binary_entry(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.parent(), right.parent()) {
        (Some(left_parent), Some(right_parent)) => {
            same_dir(left_parent, right_parent) && left.file_name() == right.file_name()
        }
        _ => false,
    }
}
