// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Test-owned runner for one isolated Live Corpus operation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use agent_semantic_provider_protocol::ProviderWorkspaceInstallDescriptor;

const SERVER_ARTIFACT_ENV: &str = "ASP_LIVE_CORPUS_SERVER_ARTIFACT";
const PROVIDER_DESCRIPTOR_ENV: &str = "ASP_LIVE_CORPUS_PROVIDER_WORKSPACE_DESCRIPTOR";

pub fn main() -> std::process::ExitCode {
    let runtime =
        match agent_semantic_workspace_scheduler::RuntimeServerRuntimeBuilder::new_client()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("failed to create Live Corpus test runtime: {error}");
                return std::process::ExitCode::from(2);
            }
        };
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result = if args.first().is_some_and(|argument| argument == "qualify") {
        runtime.block_on(run_isolated_qualification(args))
    } else {
        runtime.block_on(agent_semantic_client::live_corpus_test::run_live_corpus_test(args))
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::from(2)
        }
    }
}

async fn run_isolated_qualification(args: Vec<String>) -> Result<(), String> {
    let started = std::time::Instant::now();
    eprintln!("[live-corpus-fixture] phase=resource-bind state=starting");
    let resource_state_home = agent_semantic_runtime::resolve_state_home()?;
    let fixture = tempfile::tempdir()
        .map_err(|error| format!("create isolated Live Corpus Runtime root: {error}"))?;
    link_live_corpus_resources(&resource_state_home, fixture.path())?;
    eprintln!(
        "[live-corpus-fixture] phase=resource-bind state=ready elapsedMicros={}",
        started.elapsed().as_micros()
    );
    let publish_started = std::time::Instant::now();
    eprintln!("[live-corpus-fixture] phase=runtime-bundle state=starting");
    let server_artifact = required_artifact(SERVER_ARTIFACT_ENV)?;
    let resource = qualification_resource(&args, &resource_state_home)?;
    publish_test_runtime_bundle(&server_artifact, &resource, fixture.path()).await?;
    eprintln!(
        "[live-corpus-fixture] phase=runtime-bundle state=ready elapsedMicros={}",
        publish_started.elapsed().as_micros()
    );
    let scenario_started = std::time::Instant::now();
    eprintln!("[live-corpus-fixture] phase=server-and-scenario state=starting");
    let result =
        agent_semantic_client::live_corpus_test::run_live_corpus_test_at(args, fixture.path())
            .await;
    eprintln!(
        "[live-corpus-fixture] phase=server-and-scenario state={} elapsedMicros={}",
        if result.is_ok() { "ready" } else { "failed" },
        scenario_started.elapsed().as_micros()
    );
    stop_test_runtime(&server_artifact, fixture.path());
    result
}

#[cfg(unix)]
fn link_live_corpus_resources(
    source_state_home: &Path,
    fixture_state_home: &Path,
) -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let source = source_state_home.join("resources/live-corpus");
    if !source.is_dir() {
        return Err(format!(
            "Live Corpus test resource root is unavailable: {}",
            source.display()
        ));
    }
    let target = fixture_state_home.join("resources/live-corpus");
    std::fs::create_dir_all(target.parent().expect("resource target parent"))
        .map_err(|error| format!("create isolated resource parent: {error}"))?;
    symlink(&source, &target).map_err(|error| {
        format!(
            "link immutable Live Corpus resources into isolated Runtime: source={} target={} error={error}",
            source.display(),
            target.display()
        )
    })
}

#[cfg(not(unix))]
fn link_live_corpus_resources(
    _source_state_home: &Path,
    _fixture_state_home: &Path,
) -> Result<(), String> {
    Err("isolated Live Corpus resource binding currently requires Unix symlinks".to_owned())
}

async fn publish_test_runtime_bundle(
    server_artifact: &Path,
    resource: &LiveCorpusResource,
    fixture_state_home: &Path,
) -> Result<(), String> {
    use agent_semantic_artifacts::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure;
    use agent_semantic_artifacts::runtime_artifact_execution_closure::{
        LanguageSchemaClosureEntry, NamedRuntimeDigestClosureEntry,
    };
    use agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource;

    let asp_digest =
        agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(
            server_artifact,
        )
        .await?;
    let mut executable_members = BTreeMap::new();
    executable_members.insert("asp".to_owned(), asp_digest);
    let staging = fixture_state_home.join("runtime/live-corpus-fixture-closure");
    std::fs::create_dir_all(&staging)
        .map_err(|error| format!("create Runtime fixture closure root: {error}"))?;
    let provider_launcher = materialize_provider_launcher(resource, &staging)?;
    if let Some(provider_launcher) = provider_launcher.as_ref() {
        let digest =
            agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(
                provider_launcher,
            )
            .await?;
        executable_members.insert(resource.provider_id.clone(), digest);
    }
    let closure = RuntimeArtifactExecutionClosure::from_runtime_bundle_members(
        &executable_members,
        vec![NamedRuntimeDigestClosureEntry {
            id: "live-corpus-runtime-admission-v1".to_owned(),
            digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                    b"live-corpus-runtime-admission-v1",
                ),
        }],
        vec![NamedRuntimeDigestClosureEntry {
            id: "live-corpus-client-frame-v1".to_owned(),
            digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                    b"live-corpus-client-frame-v1",
                ),
        }],
        vec![LanguageSchemaClosureEntry {
            language_id: resource.language.clone(),
            schema_digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                    format!("live-corpus-language-schema-v1:{}", resource.language).as_bytes(),
                ),
        }],
    )?;
    let mut owned_sources = provider_launcher
        .map(|path| vec![(resource.provider_id.clone(), path)])
        .unwrap_or_default();
    for (name, bytes) in closure.materialized_members()? {
        let path = staging.join(name);
        std::fs::write(&path, bytes)
            .map_err(|error| format!("write Runtime fixture closure {name}: {error}"))?;
        owned_sources.push((name.to_owned(), path));
    }
    let member_sources = owned_sources
        .iter()
        .map(|(name, source)| RuntimeArtifactBundleMemberSource {
            name: name.as_str(),
            source,
        })
        .collect::<Vec<_>>();
    let target = fixture_state_home.join("runtime/bin/asp");
    agent_semantic_artifacts::runtime_artifact_publication::
        publish_runtime_artifact_bound_bundle_members(
            fixture_state_home,
            server_artifact,
            &target,
            "dev",
            &member_sources,
            &closure.binding()?,
        )
        .await?;
    Ok(())
}

fn stop_test_runtime(server_artifact: &Path, state_home: &Path) {
    let output = Command::new(server_artifact)
        .env("ASP_STATE_HOME", state_home)
        .args(["server", "stop"])
        .output();
    if let Ok(output) = output
        && !output.status.success()
    {
        eprintln!(
            "isolated Live Corpus Runtime cleanup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveCorpusLock {
    corpora: Vec<LiveCorpusResource>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveCorpusResource {
    resource_id: String,
    provider_id: String,
    language: String,
}

impl LiveCorpusResource {
    fn language_id(&self) -> &str {
        &self.language
    }
}

fn qualification_resource(
    args: &[String],
    state_home: &Path,
) -> Result<LiveCorpusResource, String> {
    let resource_id = option_value(args, "--resource")
        .ok_or_else(|| "Live Corpus qualification omits --resource".to_owned())?;
    let lock_path = option_value(args, "--lock")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("benchmarks/large-library-runtime-corpora.json"));
    let lock_bytes = std::fs::read(&lock_path)
        .or_else(|_| {
            std::fs::read(
                state_home.join("resources/live-corpus/large-library-runtime-corpora.json"),
            )
        })
        .map_err(|error| {
            format!(
                "read Live Corpus resource lock {}: {error}",
                lock_path.display()
            )
        })?;
    let lock: LiveCorpusLock = serde_json::from_slice(&lock_bytes)
        .map_err(|error| format!("decode Live Corpus resource lock: {error}"))?;
    lock.corpora
        .into_iter()
        .find(|resource| resource.resource_id == resource_id)
        .ok_or_else(|| format!("Live Corpus resource is not locked: {resource_id}"))
}

fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

fn required_artifact(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os(name).map(PathBuf::from).ok_or_else(|| {
        format!("reasonKind=live-corpus-prebuilt-artifact-required environment={name}")
    })?;
    let path = path.canonicalize().map_err(|error| {
        format!(
            "resolve Live Corpus prebuilt artifact {}: {error}",
            path.display()
        )
    })?;
    if !path.is_file() {
        return Err(format!(
            "Live Corpus prebuilt artifact is not a file: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn materialize_provider_launcher(
    resource: &LiveCorpusResource,
    staging: &Path,
) -> Result<Option<PathBuf>, String> {
    if matches!(resource.language_id(), "md" | "org") {
        if std::env::var_os(PROVIDER_DESCRIPTOR_ENV).is_some() {
            return Err(format!(
                "embedded Live Corpus provider rejects executable descriptor: provider={}",
                resource.provider_id
            ));
        }
        return Ok(None);
    }
    let descriptor_path = required_artifact(PROVIDER_DESCRIPTOR_ENV)?;
    let descriptor: ProviderWorkspaceInstallDescriptor = serde_json::from_slice(
        &std::fs::read(&descriptor_path)
            .map_err(|error| format!("read provider workspace descriptor: {error}"))?,
    )
    .map_err(|error| format!("decode provider workspace descriptor: {error}"))?;
    descriptor.validate()?;
    if descriptor.language_id != resource.language_id()
        || descriptor.provider_id != resource.provider_id
    {
        return Err(format!(
            "reasonKind=live-corpus-provider-artifact-identity-mismatch resource={} expectedLanguage={} actualLanguage={} expectedProvider={} actualProvider={}",
            resource.resource_id,
            resource.language_id(),
            descriptor.language_id,
            resource.provider_id,
            descriptor.provider_id
        ));
    }
    let workspace_root = std::env::current_dir()
        .map_err(|error| format!("resolve Live Corpus workspace root: {error}"))?;
    let source_artifact_root = workspace_root
        .join(&descriptor.workspace_artifact.root)
        .canonicalize()
        .map_err(|error| format!("resolve prebuilt provider artifact root: {error}"))?;
    let artifact_root = staging.join("provider-artifact-root");
    agent_semantic_artifacts::provider_workspace_artifact::materialize_provider_workspace_artifact(
        &source_artifact_root,
        &artifact_root,
    )?;
    let (artifact_digest, _) = agent_semantic_artifacts::provider_workspace_artifact::
        provider_workspace_artifact_snapshot(&artifact_root)?;
    eprintln!(
        "[live-corpus-fixture] phase=provider-artifact state=ready provider={} digest={}",
        resource.provider_id, artifact_digest
    );
    let entrypoint = if artifact_root.is_file() && descriptor.workspace_artifact.entrypoint == "." {
        artifact_root.clone()
    } else {
        artifact_root.join(&descriptor.workspace_artifact.entrypoint)
    };
    if !entrypoint.is_file() {
        return Err(format!(
            "prebuilt provider entrypoint is missing: {}",
            entrypoint.display()
        ));
    }
    let (program, arguments) = match descriptor.workspace_artifact.launch.as_ref() {
        Some(launch) => {
            let program = if launch.program_relative_to_artifact {
                artifact_root
                    .join(&launch.program)
                    .to_string_lossy()
                    .into_owned()
            } else {
                launch.program.clone()
            };
            let arguments = launch
                .args
                .iter()
                .map(|argument| {
                    if launch.args_relative_to_artifact {
                        artifact_root.join(argument).to_string_lossy().into_owned()
                    } else {
                        argument.clone()
                    }
                })
                .collect::<Vec<_>>();
            (program, arguments)
        }
        None => (entrypoint.to_string_lossy().into_owned(), Vec::new()),
    };
    let launcher = staging.join(format!("{}.launcher", resource.provider_id));
    let mut script = format!(
        "#!/bin/sh\n# provider-artifact-digest={}\nexec {}",
        artifact_digest,
        shell_quote(&program)
    );
    for argument in arguments {
        script.push(' ');
        script.push_str(&shell_quote(&argument));
    }
    script.push_str(" \"$@\"\n");
    std::fs::write(&launcher, script)
        .map_err(|error| format!("write Live Corpus provider launcher: {error}"))?;
    set_executable(&launcher)?;
    Ok(Some(launcher))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| format!("inspect Live Corpus launcher: {error}"))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
        .map_err(|error| format!("make Live Corpus launcher executable: {error}"))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Err("Live Corpus provider launchers require Unix".to_owned())
}
