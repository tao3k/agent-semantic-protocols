//! Install command routing and pinned language provider installer.

use agent_semantic_runtime::{ensure_project_provider_lock_dir, project_runtime_state};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::hook_runtime::{run_codex_plugin_install_args, run_hook_runtime_args};
use super::install_provider_archive::{
    asset_name, binary_file_name, checksum_for_archive, download_release_archive,
    install_archive_binary, path_segment, release_asset_url, sha256_file,
};
use super::install_provider_development::{
    capture_development_artifact_provenance, development_artifact_is_authorized,
    run_development_provider_installer,
};
use super::install_provider_release::ProviderReleaseSpec;
use super::install_provider_runtime_reconcile::reconcile_registered_provider_runtime_binaries;
use super::install_provider_target::resolve_provider_binary_install_target;
use super::org_capture;

#[cfg(test)]
use super::install_provider_archive::{checksum_name, parse_sha256_checksum};

const PINNED_LANGUAGE_RELEASES_TOML: &str = include_str!("../../pinned-language-releases.toml");

#[derive(Deserialize)]
struct PinnedLanguageReleaseManifest {
    languages: BTreeMap<String, PinnedLanguageReleaseEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinnedLanguageReleaseEntry {
    provider: String,
    repo: String,
    version: String,
    download_base_url: String,
    binary: String,
    archive_prefix: Option<String>,
    archive_binary: Option<String>,
    require_native_binary: Option<bool>,
    supported_targets: Vec<String>,
    #[serde(default)]
    sha256_by_target: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum InstallScope {
    #[default]
    Global,
    Project {
        root: PathBuf,
    },
}

#[derive(Debug, Default)]
struct InstallArgs {
    scope: InstallScope,
    target: Option<String>,
    reconcile_receipt: bool,
    record_installed_receipt: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderArtifactAuthority<'a> {
    DevelopBuild { root: &'a Path },
    Develop { root: &'a Path, artifact: &'a Path },
    LockedRelease,
}

fn provider_artifact_authority<'a>(
    mode: &'a agent_semantic_config::runtime_dev::RuntimeArtifactMode,
    recorded_artifact: Option<&'a Path>,
) -> Result<ProviderArtifactAuthority<'a>, String> {
    match (mode, recorded_artifact) {
        (
            agent_semantic_config::runtime_dev::RuntimeArtifactMode::Dev { root, .. },
            Some(artifact),
        ) => Ok(ProviderArtifactAuthority::Develop {
            root: root.as_path(),
            artifact,
        }),
        (agent_semantic_config::runtime_dev::RuntimeArtifactMode::Dev { root, .. }, None) => {
            Ok(ProviderArtifactAuthority::DevelopBuild {
                root: root.as_path(),
            })
        }
        (agent_semantic_config::runtime_dev::RuntimeArtifactMode::Release, Some(_)) => Err(
            "--record-installed-receipt requires `[dev] enabled = true`; release mode admits only locked releases"
                .to_owned(),
        ),
        (agent_semantic_config::runtime_dev::RuntimeArtifactMode::Release, None) => {
            Ok(ProviderArtifactAuthority::LockedRelease)
        }
    }
}

pub(crate) fn run_install_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("binary") => run_install_binary(&args[1..]),
        Some("hook") => run_install_hook(&args[1..]),
        Some("plugin") => run_install_plugin(&args[1..]),
        Some("language") => run_install_provider(&args[1..]),
        Some("help" | "--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        None => Err(usage()),
        Some(_) => Err(usage()),
    }
}

fn run_install_binary(args: &[String]) -> Result<(), String> {
    let mut target = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                if target.is_some() {
                    return Err(
                        "asp install binary accepts exactly one --target install root".to_string(),
                    );
                }
                target = Some(args.get(index + 1).ok_or_else(usage).map(PathBuf::from)?);
                index += 2;
            }
            "help" | "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => return Err(usage()),
        }
    }
    let target = target.ok_or_else(usage)?;
    let project_root = env::current_dir()
        .map_err(|error| format!("failed to resolve current project root: {error}"))?;
    let runtime_state = agent_semantic_runtime::project_runtime_state(&project_root)?;
    super::install_binary_config_admission::admit_embedded_hook_config()?;
    let _reconciliation_guard = super::protocol_binary::ProtocolBinaryReconciliationGuard::acquire(
        &runtime_state.protocol_home,
    )?;
    let artifact_root = runtime_state.protocol_home.join("runtime/artifacts");
    let plan = super::protocol_binary::ProtocolBinaryInstallPlan::capture_for_target(
        artifact_root.clone(),
        target,
    )?;
    let installed = super::protocol_binary::ensure_protocol_binary_installed(&plan)?;
    let provider_binaries = reconcile_registered_provider_runtime_binaries(
        &runtime_state.runtime_bin_dir,
        &artifact_root,
        &runtime_state.provider_lock_dir,
    )?;
    let global_provider_catalog = super::global_provider_catalog::publish_global_provider_catalog(
        &provider_binaries.provider_receipts,
    )?;
    let active_artifact_receipt = agent_semantic_hook::rebind_active_asp_binary_receipt_if_present(
        &installed.path,
        &installed.artifact_digest,
        &runtime_state.activation_path,
    )?;
    println!(
        "[asp-install-binary] binaryPath={} binaryInstall={} binaryArtifactDigest={} digestAlgorithm=blake3-256 binaryLatest={} binaryStableEntry={} binarySwitch=atomic providerRegistrations={} providerBinaryIdentities={} providerBinariesReconciled={} providerBinariesChanged={} providerBinariesMissing={} providerReceiptsReconciled={} providerReceiptsChanged={} providerReceiptsMissing={} providerBinaryByteReads={} globalProviderCatalog={} globalProviderCatalogChangedLeafCount={} globalProviderCatalogBinaryByteReads={} globalProviderCatalogWrite={} globalProviderCatalogElapsedMicros={} activeArtifactReceipt={}",
        installed.path.display(),
        installed.status,
        installed.artifact_digest,
        installed.latest.display(),
        installed.stable_entry.display(),
        provider_binaries.registration_count,
        provider_binaries.binary_identity_count,
        provider_binaries.reconciled_count,
        provider_binaries.changed_count,
        provider_binaries.missing_count,
        provider_binaries.receipt_reconciled_count,
        provider_binaries.receipt_changed_count,
        provider_binaries.receipt_missing_count,
        provider_binaries.binary_byte_reads,
        global_provider_catalog.catalog_generation,
        global_provider_catalog.changed_leaf_count,
        global_provider_catalog.binary_byte_reads,
        global_provider_catalog.catalog_write,
        global_provider_catalog.elapsed_micros,
        active_artifact_receipt.as_str(),
    );
    Ok(())
}

fn run_install_hook(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) {
        println!("{}", install_hook_usage());
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--codex") {
        return Err(
            "Codex plugin installation uses `asp install plugin --codex [PROJECT_ROOT]`"
                .to_string(),
        );
    }

    let mut forwarded = vec!["install".to_string()];
    forwarded.extend(args.iter().cloned());
    run_hook_runtime_args(forwarded)
}

fn run_install_plugin(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) {
        return super::cli_help::print_install_plugin_help();
    }
    run_codex_plugin_install_args(args)
}

fn has_help_flag(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
}

fn run_install_provider(args: &[String]) -> Result<(), String> {
    let Some(language_id) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    if matches!(language_id, "help" | "--help" | "-h") {
        return Err(usage());
    }
    let install_args = parse_install_args(&args[1..])?;
    let target = match install_args.target {
        Some(target) => target,
        None => host_target_triple().ok_or_else(|| {
            "failed to infer host target; pass --target <target-triple>".to_string()
        })?,
    };
    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let project_root = match &install_args.scope {
        InstallScope::Global => None,
        InstallScope::Project { root } => Some(root.as_path()),
    };
    if install_args.reconcile_receipt {
        let project_root = project_root
            .ok_or_else(|| "--reconcile-receipt requires --project <ROOT>".to_string())?;
        return super::install_provider_reconcile::reconcile_provider_install_receipt(
            language_id,
            project_root,
        );
    }
    let registered_binary = agent_semantic_hook::registered_provider_binary_v1(language_id)?;
    let state_home = agent_semantic_runtime::resolve_state_home()?;
    let artifact_catalog = agent_semantic_runtime::runtime_block_on_current_thread(
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        ),
    )??;
    match provider_artifact_authority(
        artifact_catalog.mode(),
        install_args.record_installed_receipt.as_deref(),
    )? {
        ProviderArtifactAuthority::DevelopBuild { root } => {
            return run_development_provider_installer(root, language_id, &target, project_root);
        }
        ProviderArtifactAuthority::Develop { root, artifact } => {
            return record_development_provider_install(
                language_id,
                registered_binary.provider_id().as_str(),
                registered_binary.binary(),
                &target,
                &invocation_root,
                project_root,
                &install_args.scope,
                artifact,
                root,
            );
        }
        ProviderArtifactAuthority::LockedRelease => {}
    }
    let spec = provider_release(language_id)?;
    if registered_binary.provider_id().as_str() != spec.provider_id {
        return Err(format!(
            "ProviderRegistry provider drift for language `{language_id}`: registry={} release={}",
            registered_binary.provider_id().as_str(),
            spec.provider_id
        ));
    }
    if registered_binary.binary() != spec.binary {
        return Err(format!(
            "ProviderRegistry binary drift for language `{language_id}`: registry={} release={}",
            registered_binary.binary(),
            spec.binary
        ));
    }
    let rev = spec.release_version.as_str();
    validate_target(&spec, &target)?;
    let provider_binary = binary_file_name(registered_binary.binary(), &target);
    let runtime_binary_identity =
        super::protocol_binary::RuntimeBinaryIdentityV1::from_registered_provider(
            &provider_binary,
        )?;
    let install_target =
        resolve_provider_binary_install_target(&spec.language_id, &provider_binary)?;
    let (scope, provider_lock_dir, provider_package_root, scope_root) = match &install_args.scope {
        InstallScope::Global => {
            let state_root = canonical_global_provider_state_root()?;
            (
                "global",
                state_root.join("receipts"),
                state_root.join("packages"),
                state_root,
            )
        }
        InstallScope::Project { root } => {
            let provider_lock_dir = ensure_project_provider_lock_dir(root)?;
            (
                "project",
                provider_lock_dir.clone(),
                provider_lock_dir,
                root.clone(),
            )
        }
    };
    let provider_package_dir = provider_package_root
        .join(&spec.language_id)
        .join(path_segment(rev))
        .join(&target);
    let asset_name = asset_name(&spec, &target);
    let archive_source = release_asset_url(&spec, &asset_name);
    let archive_path = download_release_archive(&spec, &target, &scope_root)?;
    let published_sha256 = checksum_for_archive(&spec, &target, &scope_root)?;
    let pinned_sha256 = pinned_release_sha256(&spec, &target)?;
    if let Some(pinned_sha256) = pinned_sha256
        && pinned_sha256 != published_sha256
    {
        return Err(format!(
            "release checksum sidecar mismatch for provider {} target {target}: pinned {pinned_sha256}, published {published_sha256}",
            spec.provider_id
        ));
    }
    let checksum_authority = if pinned_sha256.is_some() {
        "pinned-release+sidecar"
    } else {
        "release-sidecar"
    };
    let expected_sha256 = pinned_sha256.unwrap_or(&published_sha256);
    let actual_sha256 = sha256_file(&archive_path)?;
    if expected_sha256 != actual_sha256 {
        return Err(format!(
            "checksum mismatch for {}: expected {expected_sha256}, got {actual_sha256}",
            archive_path.display()
        ));
    }
    let installed_entrypoint = install_archive_binary(
        &archive_path,
        &spec,
        &target,
        &install_target.path,
        &provider_package_dir,
    )?;
    let runtime_state = project_runtime_state(project_root.unwrap_or(&invocation_root))?;
    let stable_entry = project_root.map_or_else(
        || install_target.path.clone(),
        |_| runtime_state.runtime_bin_dir.join(&provider_binary),
    );
    let artifact_root = runtime_state.protocol_home.join("runtime/artifacts");
    let published = super::protocol_binary::install_protocol_binary_target(
        &installed_entrypoint,
        &stable_entry,
        &artifact_root,
        &runtime_binary_identity,
    )?;
    let installed = published.path.clone();
    let runtime_bin_dir = stable_entry
        .parent()
        .ok_or_else(|| {
            format!(
                "provider runtime binary has no parent directory: {}",
                stable_entry.display()
            )
        })?
        .to_path_buf();
    let installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&installed)?;
    let installed_entrypoint_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed)?;
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &[installed.to_string_lossy().to_string()],
        &installed_entrypoint_digest,
    )?;
    let lock_path = provider_lock_dir.join(format!("{language_id}.lock.toml"));
    write_provider_lock(
        &lock_path,
        &ProviderInstallLock {
            schema_id: "asp.provider-install-lock.v1",
            scope,
            language_id: &spec.language_id,
            provider_id: &spec.provider_id,
            source_kind: "release",
            checkout_root: None,
            provider_source_root: None,
            repo: Some(&spec.repo),
            rev: Some(rev),
            target: &target,
            binary: registered_binary.binary(),
            installed_path: &installed,
            package_path: &provider_package_dir,
            sha256: &actual_sha256,
            source: archive_source,
            source_snapshot_root: None,
            source_snapshot_algorithm: None,
            source_leaf_count: None,
            provider_digest: None,
            build_recipe_digest: None,
            artifact_digest: None,
            artifact_leaf_count: None,
            artifact_entrypoint: None,
            artifact_entrypoint_sha256: None,
            installed_entrypoint_digest: Some(&installed_entrypoint_digest),
            installed_entrypoint_metadata_digest: &installed_entrypoint_metadata_digest,
            execution_command_digest: &execution_command_digest,
            launcher_digest: None,
        },
    )?;
    let org_state_sync = match project_root {
        Some(project_root) => org_capture::run_org_state_sync(project_root)?.status,
        None => "not-applicable",
    };
    println!(
        "[asp-install] provider={} language={} scope={} installMode=locked-release rev={} target={} binary={} sha256={} checksumAuthority={} installedPath={} installTargetSource={} lock={} runtimeBinDir={} binaryLatest={} binaryStableEntry={} binarySwitch=atomic orgState={} orgStateSync={}",
        spec.provider_id,
        spec.language_id,
        scope,
        rev,
        target,
        registered_binary.binary(),
        actual_sha256,
        checksum_authority,
        installed.display(),
        install_target.source,
        lock_path.display(),
        runtime_bin_dir.display(),
        published.latest.display(),
        published.stable_entry.display(),
        project_root
            .map(|root| root
                .join(".agent-semantic-protocols/org")
                .display()
                .to_string())
            .unwrap_or_else(|| "not-applicable".to_string()),
        org_state_sync,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_development_provider_install(
    language_id: &str,
    provider_id: &str,
    registered_binary: &str,
    target: &str,
    invocation_root: &Path,
    project_root: Option<&Path>,
    install_scope: &InstallScope,
    source_path: &Path,
    configured_dev_root: &Path,
) -> Result<(), String> {
    let dev_root = configured_dev_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize configured [dev].root {}: {error}",
            configured_dev_root.display()
        )
    })?;
    let registration = agent_semantic_hook::registered_provider_development_v1(language_id)?;
    if registration.provider_id.as_str() != provider_id
        || registration.binary.as_str() != registered_binary
    {
        return Err(format!(
            "ProviderRegistry development identity drift: language={language_id} provider={} binary={} expectedProvider={provider_id} expectedBinary={registered_binary}",
            registration.provider_id.as_str(),
            registration.binary
        ));
    }
    let provider_source_root = dev_root
        .join(&registration.development.source_root)
        .canonicalize()
        .map_err(|error| {
            format!(
                "failed to canonicalize registered provider sourceRoot {}: {error}",
                registration.development.source_root
            )
        })?;
    let source_path = if source_path.is_absolute() {
        source_path.to_path_buf()
    } else {
        invocation_root.join(source_path)
    }
    .canonicalize()
    .map_err(|error| format!("failed to canonicalize development artifact: {error}"))?;
    let state_home = agent_semantic_runtime::resolve_state_home()?
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize ASP State Home: {error}"))?;
    if !development_artifact_is_authorized(
        &provider_source_root,
        &state_home,
        registered_binary,
        registration.development.artifact_domain,
        &source_path,
    ) {
        return Err(format!(
            "developer artifact is outside its registered provider artifact domain: artifact={} devRoot={} providerSourceRoot={} artifactDomain={:?} stagingRoot={}",
            source_path.display(),
            dev_root.display(),
            provider_source_root.display(),
            registration.development.artifact_domain,
            state_home
                .join("runtime/provider-artifacts")
                .join(registered_binary)
                .join("develop")
                .display()
        ));
    }
    let provider_binary = binary_file_name(registered_binary, target);
    if source_path.file_name().and_then(|name| name.to_str()) != Some(provider_binary.as_str()) {
        return Err(format!(
            "development provider binary mismatch for language {language_id}: expected {provider_binary}, got {}",
            source_path.display()
        ));
    }
    let install_target = resolve_provider_binary_install_target(language_id, &provider_binary)?;
    let runtime_binary_identity =
        super::protocol_binary::RuntimeBinaryIdentityV1::from_registered_provider(
            &provider_binary,
        )?;
    let runtime_state = project_runtime_state(project_root.unwrap_or(invocation_root))?;
    let _reconciliation_guard = super::protocol_binary::ProtocolBinaryReconciliationGuard::acquire(
        &runtime_state.protocol_home,
    )?;
    let stable_entry = project_root.map_or_else(
        || install_target.path.clone(),
        |_| runtime_state.runtime_bin_dir.join(&provider_binary),
    );
    let artifact_root = runtime_state.protocol_home.join("runtime/artifacts");
    let published = super::protocol_binary::install_protocol_binary_target(
        &source_path,
        &stable_entry,
        &artifact_root,
        &runtime_binary_identity,
    )?;
    let installed_path = published.path;
    let package_path = installed_path.parent().ok_or_else(|| {
        format!(
            "installed provider binary has no parent directory: {}",
            installed_path.display()
        )
    })?;
    let installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&installed_path)?;
    let installed_entrypoint_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed_path)?;
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &[installed_path.to_string_lossy().to_string()],
        &installed_entrypoint_digest,
    )?;
    let provenance = capture_development_artifact_provenance(&dev_root, &registration, target)?;
    let installed_sha256 = sha256_file(&installed_path)?;
    let (scope, lock_path) = match install_scope {
        InstallScope::Global => (
            "global",
            canonical_global_provider_state_root()?
                .join("receipts")
                .join(format!("{language_id}.lock.toml")),
        ),
        InstallScope::Project { root } => (
            "project",
            ensure_project_provider_lock_dir(root)?.join(format!("{language_id}.lock.toml")),
        ),
    };
    write_provider_lock(
        &lock_path,
        &ProviderInstallLock {
            schema_id: "asp.provider-install-lock.v1",
            scope,
            language_id,
            provider_id,
            source_kind: "develop-workspace",
            checkout_root: Some(&dev_root),
            provider_source_root: Some(&provider_source_root),
            repo: None,
            rev: None,
            target,
            binary: registered_binary,
            installed_path: &installed_path,
            package_path,
            sha256: &installed_sha256,
            source: dev_root.display().to_string(),
            source_snapshot_root: Some(&provenance.source_snapshot_root),
            source_snapshot_algorithm: Some("blake3-merkle-v1"),
            source_leaf_count: Some(provenance.source_leaf_count),
            provider_digest: Some(&provenance.provider_digest),
            build_recipe_digest: Some(&provenance.build_recipe_digest),
            artifact_digest: Some(&installed_entrypoint_digest),
            artifact_leaf_count: Some(1),
            artifact_entrypoint: Some(&installed_path),
            artifact_entrypoint_sha256: Some(&installed_sha256),
            installed_entrypoint_digest: Some(&installed_entrypoint_digest),
            installed_entrypoint_metadata_digest: &installed_entrypoint_metadata_digest,
            execution_command_digest: &execution_command_digest,
            launcher_digest: None,
        },
    )?;
    let global_provider_catalog = if matches!(install_scope, InstallScope::Global) {
        let provider_binaries = reconcile_registered_provider_runtime_binaries(
            &runtime_state.runtime_bin_dir,
            &artifact_root,
            &runtime_state.provider_lock_dir,
        )?;
        Some(
            super::global_provider_catalog::publish_global_provider_catalog(
                &provider_binaries.provider_receipts,
            )?,
        )
    } else {
        None
    };
    println!(
        "[asp-install] provider={} language={} scope={} installMode=develop-workspace sourceKind=develop-workspace devRoot={} target={} binary={} sha256={} installedPath={} lock={} switch=atomic globalProviderCatalog={} globalProviderCatalogWrite={}",
        provider_id,
        language_id,
        scope,
        dev_root.display(),
        target,
        registered_binary,
        installed_sha256,
        installed_path.display(),
        lock_path.display(),
        global_provider_catalog
            .as_ref()
            .map(|publication| publication.catalog_generation.as_str())
            .unwrap_or("not-applicable"),
        global_provider_catalog
            .as_ref()
            .is_some_and(|publication| publication.catalog_write),
    );
    Ok(())
}

fn absolute_project_root(invocation_root: &Path, project_root: &Path) -> PathBuf {
    if project_root.is_absolute() {
        project_root.to_path_buf()
    } else {
        invocation_root.join(project_root)
    }
}

#[derive(clap::Parser)]
#[command(
    name = "asp install language",
    disable_version_flag = true,
    about = "Install a pinned language provider into global state or one explicit project"
)]
struct InstallCliArgs {
    /// Install into global ASP state (the default).
    #[arg(long, conflicts_with = "project")]
    global: bool,

    /// Install into the canonical state for this project root.
    #[arg(long, value_name = "ROOT", conflicts_with = "global")]
    project: Option<PathBuf>,

    #[arg(long, value_name = "TARGET")]
    target: Option<String>,

    #[arg(long)]
    reconcile_receipt: bool,

    #[arg(long, value_name = "PATH", conflicts_with = "reconcile_receipt")]
    record_installed_receipt: Option<PathBuf>,
}

fn parse_install_args(args: &[String]) -> Result<InstallArgs, String> {
    use clap::Parser as _;

    let cli = InstallCliArgs::try_parse_from(
        std::iter::once("asp install language").chain(args.iter().map(String::as_str)),
    )
    .map_err(|error| error.to_string())?;
    let scope = match (cli.global, cli.project) {
        (_, Some(root)) => {
            let invocation_root = env::current_dir()
                .map_err(|error| format!("failed to read current directory: {error}"))?;
            let absolute = absolute_project_root(&invocation_root, &root);
            let root = absolute.canonicalize().map_err(|error| {
                format!(
                    "failed to canonicalize project root {}: {error}",
                    absolute.display()
                )
            })?;
            InstallScope::Project { root }
        }
        (true, None) | (false, None) => InstallScope::Global,
    };
    Ok(InstallArgs {
        scope,
        target: cli.target,
        reconcile_receipt: cli.reconcile_receipt,
        record_installed_receipt: cli.record_installed_receipt,
    })
}

fn canonical_global_provider_state_root() -> Result<PathBuf, String> {
    let state_home = env::var_os("ASP_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".agent-semantic-protocols"))
        })
        .ok_or_else(|| "global provider install requires ASP_STATE_HOME or HOME".to_string())?;
    canonical_global_provider_state_root_from(&state_home)
}

fn canonical_global_provider_state_root_from(state_home: &Path) -> Result<PathBuf, String> {
    let provider_root = agent_semantic_runtime::provider_state_root(state_home);
    fs::create_dir_all(&provider_root).map_err(|error| {
        format!(
            "failed to create global provider state root {}: {error}",
            provider_root.display()
        )
    })?;
    provider_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize global provider state root {}: {error}",
            provider_root.display()
        )
    })
}

fn provider_release(language_id: &str) -> Result<ProviderReleaseSpec, String> {
    let mut manifest = pinned_language_release_manifest()?;
    let Some(entry) = manifest.languages.remove(language_id) else {
        let supported = manifest
            .languages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "[asp-install-error] state=locked-release-unavailable installMode=locked-release language={language_id} reason=language-not-pinned pinnedLanguages={supported}"
        ));
    };
    Ok(ProviderReleaseSpec {
        language_id: language_id.to_string(),
        provider_id: entry.provider,
        repo: entry.repo,
        release_version: entry.version,
        download_base_url: entry.download_base_url,
        archive_prefix: entry.archive_prefix.unwrap_or_else(|| entry.binary.clone()),
        archive_binary: entry.archive_binary.unwrap_or_else(|| entry.binary.clone()),
        require_native_binary: entry.require_native_binary.unwrap_or(false),
        binary: entry.binary,
        supported_targets: entry.supported_targets,
        sha256_by_target: entry.sha256_by_target,
    })
}

fn pinned_release_sha256<'a>(
    spec: &'a ProviderReleaseSpec,
    target: &str,
) -> Result<Option<&'a str>, String> {
    let Some(value) = spec.sha256_by_target.get(target) else {
        if spec.sha256_by_target.is_empty() {
            return Ok(None);
        }
        return Err(format!(
            "missing pinned release sha256 for provider {} target {target}",
            spec.provider_id
        ));
    };
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "invalid pinned release sha256 for provider {} target {target}",
            spec.provider_id
        ));
    }
    Ok(Some(value))
}

fn pinned_language_release_manifest() -> Result<PinnedLanguageReleaseManifest, String> {
    toml::from_str(PINNED_LANGUAGE_RELEASES_TOML)
        .map_err(|error| format!("failed to parse pinned language releases: {error}"))
}

fn host_target_triple() -> Option<String> {
    let arch = env::consts::ARCH;
    let os = env::consts::OS;
    match (arch, os) {
        ("aarch64", "macos") => Some("aarch64-apple-darwin".to_string()),
        ("aarch64", "linux") => Some("aarch64-unknown-linux-gnu".to_string()),
        ("x86_64", "linux") => Some("x86_64-unknown-linux-gnu".to_string()),
        ("x86_64", "windows") => Some("x86_64-pc-windows-msvc".to_string()),
        _ => None,
    }
}

fn validate_target(spec: &ProviderReleaseSpec, target: &str) -> Result<(), String> {
    if spec
        .supported_targets
        .iter()
        .any(|supported| supported == target)
    {
        return Ok(());
    }
    Err(format!(
        "unsupported target `{target}` for provider {}; supported targets: {}",
        spec.provider_id,
        spec.supported_targets.join(", ")
    ))
}

struct ProviderInstallLock<'a> {
    schema_id: &'a str,
    scope: &'a str,
    language_id: &'a str,
    provider_id: &'a str,
    source_kind: &'a str,
    checkout_root: Option<&'a Path>,
    provider_source_root: Option<&'a Path>,
    repo: Option<&'a str>,
    rev: Option<&'a str>,
    target: &'a str,
    binary: &'a str,
    installed_path: &'a Path,
    package_path: &'a Path,
    sha256: &'a str,
    source: String,
    source_snapshot_root: Option<&'a str>,
    source_snapshot_algorithm: Option<&'a str>,
    source_leaf_count: Option<usize>,
    provider_digest: Option<&'a str>,
    build_recipe_digest: Option<&'a str>,
    artifact_digest: Option<&'a str>,
    artifact_leaf_count: Option<usize>,
    artifact_entrypoint: Option<&'a Path>,
    artifact_entrypoint_sha256: Option<&'a str>,
    installed_entrypoint_digest: Option<&'a str>,
    installed_entrypoint_metadata_digest: &'a str,
    execution_command_digest: &'a str,
    launcher_digest: Option<&'a str>,
}

fn write_provider_lock(path: &Path, lock: &ProviderInstallLock<'_>) -> Result<(), String> {
    let mut contents = format!(
        "schemaId = \"{}\"\nscope = \"{}\"\nlanguage = \"{}\"\nprovider = \"{}\"\nsourceKind = \"{}\"\n",
        toml_escape(lock.schema_id),
        toml_escape(lock.scope),
        toml_escape(lock.language_id),
        toml_escape(lock.provider_id),
        toml_escape(lock.source_kind),
    );
    if let Some(repo) = lock.repo {
        contents.push_str(&format!("repo = \"{}\"\n", toml_escape(repo)));
    }
    if let Some(checkout_root) = lock.checkout_root {
        contents.push_str(&format!(
            "checkoutRoot = \"{}\"\n",
            toml_escape(&checkout_root.display().to_string())
        ));
    }
    if let Some(provider_source_root) = lock.provider_source_root {
        contents.push_str(&format!(
            "providerSourceRoot = \"{}\"\n",
            toml_escape(&provider_source_root.display().to_string())
        ));
    }
    if let Some(rev) = lock.rev {
        contents.push_str(&format!("rev = \"{}\"\n", toml_escape(rev)));
    }
    contents.push_str(&format!(
        "target = \"{}\"\nbinary = \"{}\"\ninstalledPath = \"{}\"\npackagePath = \"{}\"\nsha256 = \"{}\"\nsource = \"{}\"\n",
        toml_escape(lock.target),
        toml_escape(lock.binary),
        toml_escape(&lock.installed_path.display().to_string()),
        toml_escape(&lock.package_path.display().to_string()),
        toml_escape(lock.sha256),
        toml_escape(&lock.source),
    ));
    if let Some(value) = lock.source_snapshot_root {
        contents.push_str(&format!(
            "sourceSnapshotRoot = \"{}\"\n",
            toml_escape(value)
        ));
    }
    if let Some(value) = lock.source_snapshot_algorithm {
        contents.push_str(&format!(
            "sourceSnapshotAlgorithm = \"{}\"\n",
            toml_escape(value)
        ));
    }
    if let Some(value) = lock.source_leaf_count {
        contents.push_str(&format!("sourceLeafCount = {value}\n"));
    }
    if let Some(value) = lock.provider_digest {
        contents.push_str(&format!("providerDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(value) = lock.build_recipe_digest {
        contents.push_str(&format!("buildRecipeDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(value) = lock.artifact_digest {
        contents.push_str(&format!("artifactDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(value) = lock.artifact_leaf_count {
        contents.push_str(&format!("artifactLeafCount = {value}\n"));
    }
    if let Some(value) = lock.artifact_entrypoint {
        contents.push_str(&format!(
            "artifactEntrypoint = \"{}\"\n",
            toml_escape(&value.display().to_string())
        ));
    }
    if let Some(value) = lock.artifact_entrypoint_sha256 {
        contents.push_str(&format!(
            "artifactEntrypointSha256 = \"{}\"\n",
            toml_escape(value)
        ));
    }
    if let Some(value) = lock.installed_entrypoint_digest {
        contents.push_str(&format!(
            "installedEntrypointDigest = \"{}\"\n",
            toml_escape(value)
        ));
    }
    contents.push_str(&format!(
        "installedEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
        toml_escape(lock.installed_entrypoint_metadata_digest),
        toml_escape(lock.execution_command_digest),
    ));
    if let Some(value) = lock.launcher_digest {
        contents.push_str(&format!("launcherDigest = \"{}\"\n", toml_escape(value)));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    super::install_provider_reconcile::atomic_write_provider_lock(path, contents.as_bytes())
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn usage() -> String {
    "usage: asp install binary --target <path>\n       asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp install plugin --codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]\n       asp install language <language> [--global | --project <canonical-root>] [--target <target>]\n       scope: global is the default; project installation requires explicit --project <canonical-root>\n       release mode: plain `asp install language` resolves only the locked release artifact (installMode=locked-release)\n       develop mode: plain `asp install language` delegates to the development installer under [dev].root; [dev].root owns provider builds and installation (installMode=develop-workspace)".to_string()
}

fn install_hook_usage() -> String {
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]".to_string()
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider.rs"]
mod install_provider_tests;
