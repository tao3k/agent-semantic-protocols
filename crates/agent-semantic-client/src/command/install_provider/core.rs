//! Install command routing and pinned language provider installer.

use agent_semantic_runtime::project_runtime_state;
use clap::Parser;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::archive::asset_name;
use super::archive::binary_file_name;
use super::archive::checksum_for_archive;
use super::archive::download_release_archive;
use super::archive::install_archive_binary;
use super::archive::path_segment;
use super::archive::release_asset_url;
use super::archive::sha256_file;
use super::binary as install_provider_binary;
use super::release::ProviderReleaseSpec;
use super::target::resolve_provider_binary_install_target;
use super::workspace as install_provider_workspace;

use super::cli_support as install_provider_cli_support;
use install_provider_cli_support::usage;

#[cfg(test)]
use super::archive::checksum_name;
#[cfg(test)]
use super::archive::parse_sha256_checksum;

const PINNED_LANGUAGE_RELEASES_TOML: &str = include_str!("../../../pinned-language-releases.toml");

#[derive(Deserialize)]
struct PinnedLanguageReleaseManifest {
    languages: BTreeMap<String, PinnedLanguageReleaseEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinnedLanguageReleaseEntry {
    repo: String,
    version: String,
    download_base_url: String,
    archive_prefix: Option<String>,
    archive_binary: Option<String>,
    require_native_binary: Option<bool>,
    supported_targets: Vec<String>,
    #[serde(default)]
    sha256_by_target: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderArtifactAuthority<'a> {
    DevelopBuild { root: &'a Path },
    LockedRelease,
}

#[derive(Clone, Debug, Eq, PartialEq, Parser)]
#[command(
    name = "asp install language",
    disable_help_subcommand = true,
    disable_version_flag = true,
    about = "Publish a pinned language provider through ASP State Home"
)]
struct InstallLanguageOptions {
    #[arg(long, value_name = "TARGET")]
    target: Option<String>,
}

fn parse_install_language_options(args: &[String]) -> Result<InstallLanguageOptions, String> {
    InstallLanguageOptions::try_parse_from(
        std::iter::once("asp install language").chain(args.iter().map(String::as_str)),
    )
    .map_err(|error| error.to_string())
}

fn provider_artifact_authority<'a>(
    mode: &'a agent_semantic_config::runtime_dev::RuntimeArtifactMode,
) -> Result<ProviderArtifactAuthority<'a>, String> {
    match mode {
        agent_semantic_config::runtime_dev::RuntimeArtifactMode::Dev { root, .. } => {
            Ok(ProviderArtifactAuthority::DevelopBuild {
                root: root.as_path(),
            })
        }
        agent_semantic_config::runtime_dev::RuntimeArtifactMode::Release => {
            Ok(ProviderArtifactAuthority::LockedRelease)
        }
    }
}

pub(crate) async fn run_install_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("binary") => install_provider_binary::run_install_binary(&args[1..]).await,
        Some("hook") => install_provider_binary::run_install_hook(&args[1..]).await,
        Some("plugin") => install_provider_binary::run_install_plugin(&args[1..]).await,
        Some("language") => run_install_provider(&args[1..]).await,
        Some("help" | "--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        None => Err(usage()),
        Some(_) => Err(usage()),
    }
}

async fn run_install_provider(args: &[String]) -> Result<(), String> {
    let Some(language_id) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    if matches!(language_id, "help" | "--help" | "-h") {
        return Err(usage());
    }
    let install_args = parse_install_language_options(&args[1..])?;
    let install_registration =
        crate::command::provider_install_registry::provider_install_registration(language_id)?;
    let target = match install_args.target {
        Some(target) => target,
        None => host_target_triple().ok_or_else(|| {
            "failed to infer host target; pass --target <target-triple>".to_string()
        })?,
    };
    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_home = agent_semantic_runtime::resolve_state_home()?;
    let artifact_catalog =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await?;
    let provider_id = install_registration.provider_id.as_str();
    let registered_binary = install_registration.binary.as_str();
    match provider_artifact_authority(artifact_catalog.mode())? {
        ProviderArtifactAuthority::DevelopBuild { root } => {
            let registration = &install_registration;
            let built = install_provider_workspace::build_registered_provider_workspace(
                root,
                &registration,
            )
            .await?;
            return install_provider_workspace::record_registered_provider_workspace_install(
                language_id,
                provider_id,
                registered_binary,
                &target,
                &invocation_root,
                root,
                &registration,
                built,
            )
            .await;
        }
        ProviderArtifactAuthority::LockedRelease => {}
    }
    let spec = provider_release(language_id)?;
    let rev = spec.release_version.as_str();
    validate_target(&spec, &target)?;
    let provider_binary = binary_file_name(registered_binary, &target);
    let runtime_binary_identity =
        crate::command::protocol_binary::RuntimeBinaryIdentityV1::from_registered_provider(
            &provider_binary,
        )?;
    let install_target =
        resolve_provider_binary_install_target(&spec.language_id, &provider_binary)?;
    let scope = "state-home";
    let scope_root = canonical_provider_state_root()?;
    let provider_lock_dir = scope_root.join("receipts");
    let provider_package_root = scope_root.join("packages");
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
    let runtime_state = project_runtime_state(&invocation_root)?;
    let stable_entry = install_target.path.clone();
    let artifact_root =
        agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&runtime_state.protocol_home)
            .root()
            .to_path_buf();
    let published = super::protocol_binary::install_protocol_binary_target(
        &installed_entrypoint,
        &stable_entry,
        &artifact_root,
        &runtime_binary_identity,
    )
    .await?;
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
            binary: install_registration.binary.as_str(),
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
    let installed_provider_artifacts = Some(
        super::installed_provider_artifacts::publish_current_installed_provider_artifacts(
            &runtime_state.protocol_home,
        )?,
    );
    println!(
        "[asp-install] provider={} language={} scope={} installMode=locked-release rev={} target={} binary={} sha256={} checksumAuthority={} installedPath={} installTargetSource={} lock={} runtimeBinDir={} binaryCurrent={} binarySwitch=atomic installedProviderArtifacts={} installedProviderArtifactsWrite={} installedProviderArtifactsChangedLeaves={} installedProviderArtifactsElapsedMicros={}",
        spec.provider_id,
        spec.language_id,
        scope,
        rev,
        target,
        install_registration.binary.as_str(),
        actual_sha256,
        checksum_authority,
        installed.display(),
        install_target.source,
        lock_path.display(),
        runtime_bin_dir.display(),
        published.path.display(),
        installed_provider_artifacts
            .as_ref()
            .map(|publication| publication.generation())
            .unwrap_or("not-applicable"),
        installed_provider_artifacts
            .as_ref()
            .is_some_and(|publication| publication.artifact_write()),
        installed_provider_artifacts
            .as_ref()
            .map_or(0, |publication| publication.changed_leaf_count()),
        installed_provider_artifacts
            .as_ref()
            .map_or(0, |publication| publication.elapsed_micros()),
    );
    Ok(())
}

pub(super) fn canonical_provider_state_root() -> Result<PathBuf, String> {
    let state_home = env::var_os("ASP_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".agent-semantic-protocols"))
        })
        .ok_or_else(|| "provider installation requires ASP_STATE_HOME or HOME".to_string())?;
    canonical_provider_state_root_from(&state_home)
}

fn canonical_provider_state_root_from(state_home: &Path) -> Result<PathBuf, String> {
    let provider_root = agent_semantic_runtime::provider_state_root(state_home);
    fs::create_dir_all(&provider_root).map_err(|error| {
        format!(
            "failed to create State Home provider root {}: {error}",
            provider_root.display()
        )
    })?;
    provider_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize State Home provider root {}: {error}",
            provider_root.display()
        )
    })
}

fn provider_release(language_id: &str) -> Result<ProviderReleaseSpec, String> {
    let registrations = agent_semantic_provider_protocol::builtin_provider_registrations()?;
    let canonical_provider_id = canonical_provider_identity(language_id, &registrations)?;
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
    let archive_prefix = entry
        .archive_prefix
        .clone()
        .or_else(|| entry.archive_binary.clone())
        .unwrap_or_else(|| canonical_provider_id.clone());
    let archive_binary = entry
        .archive_binary
        .unwrap_or_else(|| archive_prefix.clone());
    Ok(ProviderReleaseSpec {
        language_id: language_id.to_string(),
        provider_id: canonical_provider_id,
        repo: entry.repo,
        release_version: entry.version,
        download_base_url: entry.download_base_url,
        archive_prefix,
        archive_binary,
        require_native_binary: entry.require_native_binary.unwrap_or(false),
        supported_targets: entry.supported_targets,
        sha256_by_target: entry.sha256_by_target,
    })
}

fn canonical_provider_identity(
    language_id: &str,
    registrations: &[agent_semantic_provider_protocol::ProviderRegistrationDocument],
) -> Result<String, String> {
    let matching = registrations
        .iter()
        .filter(|registration| registration.language_id == language_id)
        .collect::<Vec<_>>();
    let registration = match matching.as_slice() {
        [registration] => registration,
        [] => {
            return Err(format!(
                "no provider registration for language {language_id}"
            ));
        }
        _ => {
            return Err(format!(
                "multiple provider registrations for language {language_id}"
            ));
        }
    };
    Ok(registration.provider_id.clone())
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

pub(super) struct ProviderInstallLock<'a> {
    pub(super) schema_id: &'a str,
    pub(super) scope: &'a str,
    pub(super) language_id: &'a str,
    pub(super) provider_id: &'a str,
    pub(super) source_kind: &'a str,
    pub(super) checkout_root: Option<&'a Path>,
    pub(super) provider_source_root: Option<&'a Path>,
    pub(super) repo: Option<&'a str>,
    pub(super) rev: Option<&'a str>,
    pub(super) target: &'a str,
    pub(super) binary: &'a str,
    pub(super) installed_path: &'a Path,
    pub(super) package_path: &'a Path,
    pub(super) sha256: &'a str,
    pub(super) source: String,
    pub(super) source_snapshot_root: Option<&'a str>,
    pub(super) source_snapshot_algorithm: Option<&'a str>,
    pub(super) source_leaf_count: Option<usize>,
    pub(super) provider_digest: Option<&'a str>,
    pub(super) build_recipe_digest: Option<&'a str>,
    pub(super) artifact_digest: Option<&'a str>,
    pub(super) artifact_leaf_count: Option<usize>,
    pub(super) artifact_entrypoint: Option<&'a Path>,
    pub(super) artifact_entrypoint_sha256: Option<&'a str>,
    pub(super) installed_entrypoint_digest: Option<&'a str>,
    pub(super) installed_entrypoint_metadata_digest: &'a str,
    pub(super) execution_command_digest: &'a str,
    pub(super) launcher_digest: Option<&'a str>,
}

pub(super) fn write_provider_lock(
    path: &Path,
    lock: &ProviderInstallLock<'_>,
) -> Result<(), String> {
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
    super::super::provider_install_receipt::atomic_write_provider_lock(path, contents.as_bytes())
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
#[path = "../../../tests/unit/install_provider.rs"]
mod install_provider_tests;
