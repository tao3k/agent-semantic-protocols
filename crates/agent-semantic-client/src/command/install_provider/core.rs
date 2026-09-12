// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Install command routing and pinned language provider installer.

use agent_semantic_runtime::project_runtime_state;
use clap::Parser;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::archive::asset_name;
use super::archive::binary_file_name;
use super::archive::download_release_archive_to;
use super::archive::install_archive_binary;
use super::archive::materialize_archive_binary;
use super::archive::path_segment;
use super::archive::release_asset_url;
use super::archive::sha256_file;
use super::binary as install_provider_binary;
use super::release::ProviderReleaseSpec;
use super::target::resolve_provider_binary_install_target;
use super::workspace as install_provider_workspace;

use super::cli_support as install_provider_cli_support;
use install_provider_cli_support::usage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderArtifactAuthority<'a> {
    DevelopBuild { root: &'a Path },
    LockedRelease,
}

pub(super) struct PreparedActiveProviderReconciliation {
    staging_root: PathBuf,
    members: Vec<(String, PathBuf)>,
    closure_members: Vec<(String, PathBuf)>,
    predecessor_bundle: Option<(
        PathBuf,
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    )>,
}

impl PreparedActiveProviderReconciliation {
    pub(super) fn member_sources(
        &self,
    ) -> Vec<
        agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource<
            '_,
        >,
    > {
        self.members
            .iter()
            .chain(self.closure_members.iter())
            .map(|(name, source)| {
                agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
                    name,
                    source,
                }
            })
            .collect()
    }

    pub(super) fn provider_count(&self) -> usize {
        self.members.len()
    }

    pub(super) fn predecessor_bundle(
        &self,
    ) -> Option<(
        &Path,
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    )> {
        self.predecessor_bundle
            .as_ref()
            .map(|(path, digest)| (path.as_path(), digest))
    }

    pub(super) async fn bind_runtime_execution_closure(
        &mut self,
        asp_source: &Path,
        hook_source: &Path,
    ) -> Result<
        agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding,
        String,
    > {
        use agent_semantic_artifacts::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure;

        let mut bundle_members = std::collections::BTreeMap::new();
        for (name, path) in std::iter::once(("asp", asp_source))
            .chain(std::iter::once(("asp-hook", hook_source)))
            .chain(
                self.members
                    .iter()
                    .map(|(name, path)| (name.as_str(), path.as_path())),
            )
        {
            bundle_members.insert(
                name.to_owned(),
                agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(
                    path,
                )
                .await?,
            );
        }
        let mut policy = vec![
            named_digest(
                "runtime-server-workspace-generation-admission.v1",
                include_bytes!(
                    "../../../../../schemas/runtime-server-workspace-generation-admission.schema.json"
                ),
            ),
            named_digest(
                "semantic-search-engine-admission.v1",
                include_bytes!(
                    "../../../../../schemas/semantic-search-engine-admission.v1.schema.json"
                ),
            ),
        ];
        policy.sort_by(|left, right| left.id.cmp(&right.id));
        let mut abi = vec![
            named_digest(
                "asp-client-frame.v1",
                include_bytes!("../../../../../schemas/asp-client-frame.schema.json"),
            ),
            named_digest(
                "asp-client-workspace-query-playbook-request.v1",
                include_bytes!(
                    "../../../../../schemas/asp-client-workspace-query-playbook-request.v1.schema.json"
                ),
            ),
            named_digest(
                "query-playbook-materialization-receipt.v1",
                include_bytes!(
                    "../../../../../schemas/query-playbook-materialization-receipt.v1.schema.json"
                ),
            ),
            named_digest(
                "query-playbook-materialization-request.v1",
                include_bytes!(
                    "../../../../../schemas/query-playbook-materialization-request.v1.schema.json"
                ),
            ),
        ];
        abi.sort_by(|left, right| left.id.cmp(&right.id));
        let mut schemas = [
            include_str!("../../../../../languages/asp-rust/schemas/.asp-schema-manager-membership.json"),
            include_str!("../../../../../languages/asp-typescript/schemas/.asp-schema-manager-membership.json"),
            include_str!("../../../../../languages/asp-python/schemas/.asp-schema-manager-membership.json"),
            include_str!("../../../../../languages/AspJulia.jl/schemas/.asp-schema-manager-membership.json"),
            include_str!("../../../../../languages/asp-gerbil-scheme/schemas/.asp-schema-manager-membership.json"),
            include_str!("../../../../../languages/orgize/provider/org/schemas/.asp-schema-manager-membership.json"),
            include_str!("../../../../../languages/orgize/provider/md/schemas/.asp-schema-manager-membership.json"),
        ]
        .into_iter()
        .map(schema_closure_entry)
        .collect::<Result<Vec<_>, _>>()?;
        schemas.sort_by(|left, right| left.language_id.cmp(&right.language_id));
        let closure = RuntimeArtifactExecutionClosure::from_runtime_bundle_members(
            &bundle_members,
            policy,
            abi,
            schemas,
        )?;
        std::fs::create_dir_all(&self.staging_root).map_err(|error| {
            format!(
                "create Runtime execution closure staging root {}: {error}",
                self.staging_root.display()
            )
        })?;
        self.closure_members.clear();
        for (name, bytes) in closure.materialized_members()? {
            let path = self.staging_root.join(name);
            std::fs::write(&path, bytes).map_err(|error| {
                format!(
                    "write Runtime execution closure member {}: {error}",
                    path.display()
                )
            })?;
            self.closure_members.push((name.to_owned(), path));
        }
        closure.binding()
    }
}

fn named_digest(
    id: &str,
    bytes: &[u8],
) -> agent_semantic_artifacts::runtime_artifact_execution_closure::NamedRuntimeDigestClosureEntry {
    agent_semantic_artifacts::runtime_artifact_execution_closure::NamedRuntimeDigestClosureEntry {
        id: id.to_owned(),
        digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            bytes,
        ),
    }
}

fn schema_closure_entry(
    bytes: &str,
) -> Result<
    agent_semantic_artifacts::runtime_artifact_execution_closure::LanguageSchemaClosureEntry,
    String,
> {
    let value: serde_json::Value = serde_json::from_str(bytes)
        .map_err(|error| format!("decode embedded Schema Manager membership: {error}"))?;
    let language_id = value
        .get("languageId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "embedded Schema Manager membership omitted languageId".to_owned())?;
    let bundle_digest = value
        .get("bundleDigest")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "embedded Schema Manager membership omitted bundleDigest".to_owned())?;
    Ok(
        agent_semantic_artifacts::runtime_artifact_execution_closure::LanguageSchemaClosureEntry {
            language_id: language_id.to_owned(),
            schema_digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                    bundle_digest,
                )?,
        },
    )
}

impl Drop for PreparedActiveProviderReconciliation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.staging_root);
    }
}

pub(super) fn prepare_active_provider_reconciliation(
    state_home: &Path,
) -> Result<PreparedActiveProviderReconciliation, String> {
    let staging_root = std::env::temp_dir().join(format!(
        "asp-provider-reconciliation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("system time before Unix epoch: {error}"))?
            .as_nanos()
    ));
    let mut reconciliation = PreparedActiveProviderReconciliation {
        staging_root,
        members: Vec::new(),
        closure_members: Vec::new(),
        predecessor_bundle: None,
    };
    let developer_root =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_developer_root(
            state_home,
        )?;
    let active_slot = agent_semantic_artifacts::RuntimeArtifactStateLayout::new(state_home)
        .active_slot()
        .to_path_buf();
    if let Err(error) = std::fs::symlink_metadata(&active_slot) {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(reconciliation);
        }
        return Err(format!(
            "reasonKind=runtime-active-generation-unavailable path={} error={error}",
            active_slot.display()
        ));
    }
    let active =
        agent_semantic_artifacts::load_active_runtime_bound_provider_set_for_binary_replacement(
            state_home,
        )?;
    reconciliation.predecessor_bundle = Some((
        active.runtime_bundle_path.clone(),
        active.runtime_bundle_digest.clone(),
    ));
    if active.providers.is_empty() {
        return Ok(reconciliation);
    }
    if let Some(developer_root) = developer_root.as_deref() {
        reconciliation.members = active
            .providers
            .into_iter()
            .map(|provider| {
                let source = if provider.registration_digest_matches_current {
                    provider.materialized_path
                } else {
                    super::reconciliation::qualified_staged_development_provider(
                        state_home,
                        developer_root,
                        &provider.language_id,
                        &provider.provider_id,
                    )?
                };
                Ok((provider.provider_id, source))
            })
            .collect::<Result<Vec<_>, String>>()?;
        reconciliation
            .members
            .sort_by(|left, right| left.0.cmp(&right.0));
        return Ok(reconciliation);
    }
    let target = host_target_triple().ok_or_else(|| {
        "reasonKind=provider-release-host-target-unsupported automatic Provider reconciliation requires a supported host target".to_owned()
    })?;
    reconciliation.members.reserve(active.providers.len());
    for provider in active.providers {
        let spec = provider_release(&provider.language_id)?;
        if spec.provider_id != provider.provider_id {
            return Err(format!(
                "reasonKind=provider-release-active-identity-mismatch languageId={} activeProvider={} catalogProvider={}",
                provider.language_id, provider.provider_id, spec.provider_id
            ));
        }
        validate_target(&spec, &target)?;
        let provider_root = reconciliation.staging_root.join(&provider.language_id);
        let archive = download_release_archive_to(&spec, &target, &provider_root)?;
        let expected = pinned_release_sha256(&spec, &target)?;
        let actual = sha256_file(&archive)?;
        if actual != expected {
            return Err(format!(
                "reasonKind=provider-release-artifact-digest-mismatch provider={} target={} expected={} actual={}",
                spec.provider_id, target, expected, actual
            ));
        }
        let source =
            materialize_archive_binary(&archive, &spec, &target, &provider_root.join("package"))?;
        reconciliation.members.push((spec.provider_id, source));
    }
    reconciliation
        .members
        .sort_by(|left, right| left.0.cmp(&right.0));
    Ok(reconciliation)
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
    let target = match install_args.target {
        Some(target) => target,
        None => host_target_triple().ok_or_else(|| {
            "failed to infer host target; pass --target <target-triple>".to_string()
        })?,
    };
    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_home = agent_semantic_runtime::resolve_state_home()?;
    let artifact_mode =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_artifact_mode(
            &state_home,
        )?;
    match provider_artifact_authority(&artifact_mode)? {
        ProviderArtifactAuthority::DevelopBuild { root } => {
            let install_registration =
                crate::command::provider_install_registry::provider_install_registration(
                    language_id,
                )?;
            let provider_id = install_registration.provider_id.as_str();
            let registered_binary = install_registration.binary.as_str();
            let registration = &install_registration;
            let built =
                install_provider_workspace::build_registered_provider_workspace(root, registration)
                    .await?;
            return install_provider_workspace::record_registered_provider_workspace_install(
                language_id,
                provider_id,
                registered_binary,
                &target,
                &invocation_root,
                root,
                registration,
                built,
            )
            .await;
        }
        ProviderArtifactAuthority::LockedRelease => {}
    }
    let spec = provider_release(language_id)?;
    let registered_binary = spec.binary.as_str();
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
    let archive_path = download_release_archive_to(&spec, &target, &scope_root.join("downloads"))?;
    let expected_sha256 = pinned_release_sha256(&spec, &target)?;
    let checksum_authority = "embedded-release-catalog";
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
    let execution_command_digest =
        agent_semantic_content_identity::provider_execution_command_digest(
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
            binary: registered_binary,
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
    println!(
        "[asp-install] provider={} language={} scope={} installMode=locked-release rev={} target={} binary={} sha256={} checksumAuthority={} installedPath={} installTargetSource={} lock={} runtimeBinDir={} binaryCurrent={} binarySwitch=atomic activeRuntimeBundleDigest={}",
        spec.provider_id,
        spec.language_id,
        scope,
        rev,
        target,
        registered_binary,
        actual_sha256,
        checksum_authority,
        installed.display(),
        install_target.source,
        lock_path.display(),
        runtime_bin_dir.display(),
        published.path.display(),
        published.bundle_digest.as_deref().unwrap_or("unavailable"),
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
    let provider_root =
        agent_semantic_artifacts::RuntimeArtifactStateLayout::new(state_home).provider_staging();
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
    let mut catalog = provider_release_catalog()?;
    let Some(entry) = catalog.releases.remove(language_id) else {
        let supported = catalog
            .releases
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "[asp-install-error] state=locked-release-unavailable installMode=locked-release language={language_id} reason=language-not-pinned pinnedLanguages={supported}"
        ));
    };
    if entry.provider_id != canonical_provider_id {
        return Err(format!(
            "provider release identity differs from the provider capability registry: languageId={language_id} releaseProviderId={} registeredProviderId={canonical_provider_id}",
            entry.provider_id
        ));
    }
    let archive_prefix = entry
        .archive_prefix
        .clone()
        .or_else(|| entry.archive_binary.clone())
        .unwrap_or_else(|| entry.binary.clone());
    let archive_binary = entry
        .archive_binary
        .unwrap_or_else(|| archive_prefix.clone());
    Ok(ProviderReleaseSpec {
        language_id: language_id.to_string(),
        provider_id: entry.provider_id,
        binary: entry.binary,
        repo: entry.repo,
        release_version: entry.release_version,
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
) -> Result<&'a str, String> {
    let Some(value) = spec.sha256_by_target.get(target) else {
        return Err(format!(
            "reasonKind=provider-release-target-digest-missing provider={} target={target}",
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
    Ok(value)
}

fn provider_release_catalog()
-> Result<agent_semantic_provider_protocol::ProviderReleaseCatalog, String> {
    agent_semantic_provider_protocol::builtin_provider_release_catalog()
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
