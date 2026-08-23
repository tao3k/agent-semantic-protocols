//! Immutable, process-independent Hook matcher snapshot publication and loading.

use agent_semantic_hook::{
    ClientHookConfig, DecisionKind, DurableHookConfigArtifact, HookDecision,
};
use memmap2::MmapOptions;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const ACTIVE_MATCHER_ARTIFACT: &str = "active-matcher.v1.bin";
const ACTIVE_MATCHER_MAGIC: &[u8; 8] = b"ASPHK1PC";
const ACTIVE_MATCHER_HEADER_LEN: usize = 8 + 8 + 16 + 8 + 16 + 64;
const ACTIVE_MATCHER_SECTION_ENTRY_LEN: usize = 1 + 8 + 8 + 8 + 32;
static ACTIVE_MATCHER_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
#[path = "../../tests/unit/command/hook_matcher_binary_v1.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum MatcherSectionKind {
    Complete = 0,
    DirectRead = 1,
    ShellRead = 2,
    ShellCommand = 3,
    StructuredProjection = 4,
}

struct MatcherSection {
    kind: MatcherSectionKind,
    key: [u8; 8],
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceStamp {
    byte_len: u64,
    modified_nanos: u128,
}

fn source_stamp(path: &Path) -> Result<SourceStamp, String> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(SourceStamp {
            byte_len: metadata.len(),
            modified_nanos: metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |duration| duration.as_nanos()),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SourceStamp {
            byte_len: 0,
            modified_nanos: 0,
        }),
        Err(error) => Err(format!(
            "stat Hook matcher source {}: {error}",
            path.display()
        )),
    }
}

fn matcher_cache_dir(project_root: &Path) -> Result<PathBuf, String> {
    // Hook is an independent, one-shot execution plane. Its read-only policy
    // cache must not pay Runtime/checkout identity discovery on every Host
    // action. The canonical workspace path is sufficient cache identity; the
    // generation content still binds the complete config and agent owners.
    let normalized_project_root = agent_semantic_hook::normalize_workspace_path(project_root);
    let workspace_key = format!(
        "{:x}",
        Sha256::digest(normalized_project_root.as_os_str().as_encoded_bytes())
    );
    Ok(hook_state_home()?
        .join("hooks")
        .join("cache")
        .join("workspaces")
        .join(&workspace_key[..16])
        .join("compiled-matchers"))
}

pub(crate) fn hook_state_home() -> Result<PathBuf, String> {
    if let Some(state_home) = std::env::var_os("ASP_STATE_HOME") {
        if state_home.is_empty() {
            return Err("ASP_STATE_HOME is set but empty".to_owned());
        }
        return Ok(PathBuf::from(state_home));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_owned())?;
    if home.is_empty() {
        return Err("HOME is set but empty".to_owned());
    }
    Ok(PathBuf::from(home).join(".agent-semantic-protocols"))
}

fn active_matcher_path(project_root: &Path) -> Result<PathBuf, String> {
    Ok(matcher_cache_dir(project_root)?.join(ACTIVE_MATCHER_ARTIFACT))
}

fn matcher_section_key(value: &str) -> [u8; 8] {
    Sha256::digest(value.as_bytes())[..8]
        .try_into()
        .expect("SHA-256 prefix is eight bytes")
}

fn current_source_stamps(
    config_path: &Path,
    project_root: &Path,
) -> Result<(SourceStamp, SourceStamp), String> {
    Ok((
        source_stamp(config_path)?,
        source_stamp(&agent_semantic_hook::project_agent_config_path(
            project_root,
        ))?,
    ))
}

fn take_u64(bytes: &[u8], offset: &mut usize) -> Result<u64, String> {
    let value = bytes
        .get(*offset..*offset + 8)
        .ok_or_else(|| "Hook active matcher header is truncated".to_owned())?;
    *offset += 8;
    Ok(u64::from_le_bytes(value.try_into().expect("eight bytes")))
}

fn take_u128(bytes: &[u8], offset: &mut usize) -> Result<u128, String> {
    let value = bytes
        .get(*offset..*offset + 16)
        .ok_or_else(|| "Hook active matcher header is truncated".to_owned())?;
    *offset += 16;
    Ok(u128::from_le_bytes(
        value.try_into().expect("sixteen bytes"),
    ))
}

fn take_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, String> {
    let value = bytes
        .get(*offset..*offset + 4)
        .ok_or_else(|| "Hook matcher Binary v1 section count is truncated".to_owned())?;
    *offset += 4;
    Ok(u32::from_le_bytes(value.try_into().expect("four bytes")))
}

fn select_matcher_section<'a>(
    bundle: &'a [u8],
    kind: MatcherSectionKind,
    key: [u8; 8],
) -> Result<Option<&'a [u8]>, String> {
    let mut offset = 0;
    let count = usize::try_from(take_u32(bundle, &mut offset)?)
        .map_err(|_| "Hook matcher Binary v1 section count overflow".to_owned())?;
    let index_len = count
        .checked_mul(ACTIVE_MATCHER_SECTION_ENTRY_LEN)
        .ok_or_else(|| "Hook matcher Binary v1 index length overflow".to_owned())?;
    let payload_start = offset
        .checked_add(index_len)
        .filter(|end| *end <= bundle.len())
        .ok_or_else(|| "Hook matcher Binary v1 section index is truncated".to_owned())?;
    for _ in 0..count {
        let encoded_kind = *bundle
            .get(offset)
            .ok_or_else(|| "Hook matcher Binary v1 section kind is truncated".to_owned())?;
        offset += 1;
        let encoded_key: [u8; 8] = bundle
            .get(offset..offset + 8)
            .ok_or_else(|| "Hook matcher Binary v1 section key is truncated".to_owned())?
            .try_into()
            .expect("eight-byte section key");
        offset += 8;
        let relative_start = usize::try_from(take_u64(bundle, &mut offset)?)
            .map_err(|_| "Hook matcher Binary v1 section offset overflow".to_owned())?;
        let byte_len = usize::try_from(take_u64(bundle, &mut offset)?)
            .map_err(|_| "Hook matcher Binary v1 section length overflow".to_owned())?;
        let expected_digest = bundle
            .get(offset..offset + 32)
            .ok_or_else(|| "Hook matcher Binary v1 section digest is truncated".to_owned())?;
        offset += 32;
        if encoded_kind != kind as u8 || encoded_key != key {
            continue;
        }
        let start = payload_start
            .checked_add(relative_start)
            .ok_or_else(|| "Hook matcher Binary v1 section start overflow".to_owned())?;
        let end = start
            .checked_add(byte_len)
            .filter(|end| *end <= bundle.len())
            .ok_or_else(|| "Hook matcher Binary v1 section payload is truncated".to_owned())?;
        let payload = &bundle[start..end];
        if expected_digest != blake3::hash(payload).as_bytes() {
            return Err("Hook matcher Binary v1 section digest mismatch".to_owned());
        }
        return Ok(Some(payload));
    }
    Ok(None)
}

fn load_active_matcher(
    project_root: &Path,
    direct_read_extension: Option<&str>,
    direct_read_path: Option<&str>,
    shell_read_keys: &[agent_semantic_hook::ShellReadSourceKey],
) -> Result<Option<LoadedHookConfig>, String> {
    let started = std::time::Instant::now();
    let trace_enabled = std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some();
    let trace = |stage: &str| {
        if trace_enabled {
            eprintln!(
                "[asp-hook] route=active-matcher stage={stage} elapsedMicros={}",
                started.elapsed().as_micros()
            );
        }
    };
    let path = active_matcher_path(project_root)?;
    trace("path");
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "open Hook active matcher {}: {error}",
                path.display()
            ));
        }
    };
    trace("open");
    // SAFETY: active artifacts are published by atomic rename and never
    // mutated in place. A concurrent publisher cannot invalidate this mapping.
    let mapped = unsafe { MmapOptions::new().map(&file) }
        .map_err(|error| format!("mmap Hook active matcher {}: {error}", path.display()))?;
    trace("mmap");
    if mapped.len() < ACTIVE_MATCHER_HEADER_LEN
        || mapped.get(..8) != Some(ACTIVE_MATCHER_MAGIC.as_slice())
    {
        return Err(format!(
            "Hook active matcher header is invalid: {}",
            path.display()
        ));
    }
    let mut offset = 8;
    let published_config_byte_len = take_u64(&mapped, &mut offset)?;
    let published_config_modified_nanos = take_u128(&mapped, &mut offset)?;
    let published_agents_byte_len = take_u64(&mapped, &mut offset)?;
    let published_agents_modified_nanos = take_u128(&mapped, &mut offset)?;
    let generation = mapped
        .get(offset..offset + 64)
        .ok_or_else(|| "Hook active matcher generation is truncated".to_owned())?;
    if !generation.iter().all(u8::is_ascii_hexdigit) {
        return Err("Hook active matcher generation is invalid".to_owned());
    }
    offset += 64;
    // The active artifact is an immutable, atomically published generation.
    // Hook data-path readers consume that generation directly; config/agent
    // source freshness belongs to the control-plane publisher. Per-request
    // stat/canonicalization would turn every Host action into hidden workspace
    // discovery and reintroduce the deadlock/performance failure this cache
    // exists to prevent.
    // Source stamps are publication receipts, not read-side admission checks.
    // The control-plane publisher validates and atomically replaces the whole
    // matcher generation. Re-statting source files here would make every Host
    // action race mutable source state and duplicate the publisher authority.
    let _publication_receipt = (
        published_config_byte_len,
        published_config_modified_nanos,
        published_agents_byte_len,
        published_agents_modified_nanos,
    );
    let bundle = &mapped[offset..];
    let mut shell_read_allow: Option<HookDecision> = None;
    for key in shell_read_keys {
        if let Some(table) = select_matcher_section(
            bundle,
            MatcherSectionKind::ShellRead,
            matcher_section_key(&key.extension),
        )? {
            if let Some(mut decision) =
                agent_semantic_hook::CommandDecisionShard::select_for_command(
                    table,
                    &key.command,
                    &key.command_tokens,
                )?
            {
                let placeholder = format!("__ASP_SHELL_READ_PATH__{}", key.extension);
                if !decision.replace_template_marker(&placeholder, &key.path) {
                    return Err(
                        "shell-read Hook decision shard omitted its source placeholder".to_owned(),
                    );
                }
                decision.subject.command = Some(key.command.clone());
                decision.subject.tool_name = Some(key.tool_name.clone());
                if decision.decision == DecisionKind::Deny {
                    trace("shell-decision-shard");
                    return Ok(Some(LoadedHookConfig {
                        config: None,
                        decision: Some(decision),
                        projection: Some("shell-read-decision-shard"),
                    }));
                }
                shell_read_allow.get_or_insert(decision);
            }
        }
    }
    if let Some(decision) = shell_read_allow {
        trace("shell-decision-shard");
        return Ok(Some(LoadedHookConfig {
            config: None,
            decision: Some(decision),
            projection: Some("shell-read-decision-shard"),
        }));
    }
    // Shell shards cache parser-owned action facts only. Finalizing an allow
    // or deny here would bypass the complete Config Rule DSL and its
    // cross-rule dominance. Keep using this mapping and hydrate the complete
    // matcher below instead of reopening and remapping the same generation.
    if let (Some(extension), Some(source_path)) = (direct_read_extension, direct_read_path) {
        if let Some(template) = select_matcher_section(
            bundle,
            MatcherSectionKind::DirectRead,
            matcher_section_key(extension),
        )? {
            let placeholder = format!("__ASP_DIRECT_READ_PATH__{extension}");
            let mut decision = agent_semantic_hook::HookDecision::from_compact_binary(template)?;
            if !decision.replace_template_marker(&placeholder, source_path) {
                return Err(
                    "direct-read Hook decision shard omitted its source placeholder".to_owned(),
                );
            }
            trace("decision-shard");
            return Ok(Some(LoadedHookConfig {
                config: None,
                decision: Some(decision),
                projection: Some("direct-read-decision-shard"),
            }));
        }
    }
    let artifact = select_matcher_section(bundle, MatcherSectionKind::Complete, [0; 8])?
        .ok_or_else(|| "Hook matcher Binary v1 omitted its complete matcher section".to_owned())?;
    let artifact = DurableHookConfigArtifact::from_binary_bytes(artifact)
        .map_err(|error| format!("decode Hook active matcher {}: {error}", path.display()))?;
    trace("decode");
    let config = ClientHookConfig::from_durable_snapshot_config(artifact)
        .map_err(|error| format!("hydrate Hook active matcher {}: {error}", path.display()))?;
    trace("hydrate");
    Ok(Some(LoadedHookConfig {
        config: Some(config),
        decision: None,
        projection: None,
    }))
}

fn encode_matcher_bundle(sections: &[MatcherSection]) -> Result<Vec<u8>, String> {
    let count = u32::try_from(sections.len())
        .map_err(|_| "Hook matcher Binary v1 has too many sections".to_owned())?;
    let index_len = sections
        .len()
        .checked_mul(ACTIVE_MATCHER_SECTION_ENTRY_LEN)
        .and_then(|length| length.checked_add(4))
        .ok_or_else(|| "Hook matcher Binary v1 index length overflow".to_owned())?;
    let payload_len = sections
        .iter()
        .try_fold(0usize, |total, section| {
            total.checked_add(section.bytes.len())
        })
        .ok_or_else(|| "Hook matcher Binary v1 payload length overflow".to_owned())?;
    let mut bundle = Vec::with_capacity(index_len + payload_len);
    bundle.extend_from_slice(&count.to_le_bytes());
    let mut relative_start = 0_u64;
    for section in sections {
        let byte_len = u64::try_from(section.bytes.len())
            .map_err(|_| "Hook matcher Binary v1 section length overflow".to_owned())?;
        bundle.push(section.kind as u8);
        bundle.extend_from_slice(&section.key);
        bundle.extend_from_slice(&relative_start.to_le_bytes());
        bundle.extend_from_slice(&byte_len.to_le_bytes());
        bundle.extend_from_slice(blake3::hash(&section.bytes).as_bytes());
        relative_start = relative_start
            .checked_add(byte_len)
            .ok_or_else(|| "Hook matcher Binary v1 payload offset overflow".to_owned())?;
    }
    for section in sections {
        bundle.extend_from_slice(&section.bytes);
    }
    Ok(bundle)
}

fn publish_active_matcher(
    config_path: &Path,
    project_root: &Path,
    generation: &str,
    compiled: &ClientHookConfig,
) -> Result<(), String> {
    if generation.len() != 64 || !generation.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err("Hook matcher generation must be a 64-byte hexadecimal digest".to_owned());
    }
    let (config, agents) = current_source_stamps(config_path, project_root)?;
    let mut sections = vec![MatcherSection {
        kind: MatcherSectionKind::Complete,
        key: [0; 8],
        bytes: compiled.durable_snapshot_config().to_binary_bytes()?,
    }];
    let shards = compiled.materialized_decision_shards()?;
    for (extension, bytes) in shards.direct_read {
        sections.push(MatcherSection {
            kind: MatcherSectionKind::DirectRead,
            key: matcher_section_key(&extension),
            bytes,
        });
    }
    for (extension, bytes) in shards.shell_read {
        sections.push(MatcherSection {
            kind: MatcherSectionKind::ShellRead,
            key: matcher_section_key(&extension),
            bytes,
        });
    }
    sections.push(MatcherSection {
        kind: MatcherSectionKind::StructuredProjection,
        key: [0; 8],
        bytes: shards.structured_projection,
    });
    sections.push(MatcherSection {
        kind: MatcherSectionKind::ShellCommand,
        key: [0; 8],
        bytes: shards.shell_command,
    });
    let bundle = encode_matcher_bundle(&sections)?;
    publish_active_artifact(
        &active_matcher_path(project_root)?,
        generation,
        &config,
        &agents,
        &bundle,
    )
}

fn publish_active_artifact(
    path: &Path,
    generation: &str,
    config: &SourceStamp,
    agents: &SourceStamp,
    artifact: &[u8],
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Hook active matcher has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Hook active matcher directory {}: {error}",
            parent.display()
        )
    })?;
    let mut bytes = Vec::with_capacity(ACTIVE_MATCHER_HEADER_LEN + artifact.len());
    bytes.extend_from_slice(ACTIVE_MATCHER_MAGIC);
    bytes.extend_from_slice(&config.byte_len.to_le_bytes());
    bytes.extend_from_slice(&config.modified_nanos.to_le_bytes());
    bytes.extend_from_slice(&agents.byte_len.to_le_bytes());
    bytes.extend_from_slice(&agents.modified_nanos.to_le_bytes());
    bytes.extend_from_slice(generation.as_bytes());
    bytes.extend_from_slice(artifact);
    let temp = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(ACTIVE_MATCHER_ARTIFACT),
        std::process::id(),
        ACTIVE_MATCHER_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    let mut publish = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| format!("create Hook active matcher {}: {error}", temp.display()))?;
    publish
        .write_all(&bytes)
        .and_then(|()| publish.sync_all())
        .map_err(|error| format!("sync Hook active matcher {}: {error}", temp.display()))?;
    fs::rename(&temp, &path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!(
            "publish Hook active matcher {} -> {}: {error}",
            temp.display(),
            path.display()
        )
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "sync Hook active matcher directory {}: {error}",
                parent.display()
            )
        })
}

pub(crate) struct LoadedHookConfig {
    pub(crate) config: Option<ClientHookConfig>,
    pub(crate) decision: Option<agent_semantic_hook::HookDecision>,
    pub(crate) projection: Option<&'static str>,
}

fn recovery_instruction() -> &'static str {
    "automatic Hook matcher publication could not recover; repair invalid source config or atomically publish a validated candidate with `<candidate-asp> install binary` through the Host bootstrap channel; Runtime configuration owns the stable install slot"
}

fn append_file_identity(hasher: &mut Sha256, path: &Path) -> Result<(), String> {
    hasher.update(path.as_os_str().as_encoded_bytes());
    match fs::read(path) {
        Ok(bytes) => {
            hasher.update([1]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update([0]),
        Err(error) => {
            return Err(format!(
                "read Hook matcher generation input {}: {error}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn compiled_generation_key(config_path: &Path, project_root: &Path) -> Result<String, String> {
    compiled_generation_key_with_artifact_fingerprint(
        config_path,
        project_root,
        &agent_semantic_hook::hook_runtime_artifact_fingerprint(),
    )
}

fn compiled_generation_key_with_artifact_fingerprint(
    config_path: &Path,
    project_root: &Path,
    artifact_fingerprint: &str,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(b"asp-hook-compiled-mmap-generation-v2\0");
    hasher.update(b"hook-matcher-compiler-v2-action-rule-dominance\0");
    hasher.update(project_root.as_os_str().as_encoded_bytes());
    hasher.update(agent_semantic_config::hook_client_contract_fingerprint().as_bytes());
    hasher.update(artifact_fingerprint.as_bytes());
    append_file_identity(&mut hasher, config_path)?;
    append_file_identity(
        &mut hasher,
        &agent_semantic_hook::project_agent_config_path(project_root),
    )?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn load_fresh_hook_config(
    config_path: &Path,
    project_root: &Path,
    direct_read_extension: Option<&str>,
    direct_read_path: Option<&str>,
    shell_read_keys: &[agent_semantic_hook::ShellReadSourceKey],
    shell_command_keys: &[agent_semantic_hook::ShellCommandKey],
) -> Result<(LoadedHookConfig, &'static str), String> {
    let load_requested = || {
        load_active_matcher(
            project_root,
            direct_read_extension,
            direct_read_path,
            shell_read_keys,
        )
    };
    let has_specialized_request = direct_read_extension.is_some()
        || !shell_read_keys.is_empty()
        || !shell_command_keys.is_empty();
    let load_complete = || load_active_matcher(project_root, None, None, &[]);

    match load_requested() {
        Ok(Some(loaded)) => return Ok((loaded, "mmap-hit")),
        Ok(None) if has_specialized_request => {
            if let Ok(Some(loaded)) = load_complete() {
                return Ok((loaded, "mmap-hit"));
            }
        }
        Ok(None) | Err(_) => {}
    }

    publish_hook_matcher_generation(config_path, project_root).map_err(|error| {
        format!(
            "Hook matcher Binary v1 automatic publication failed for {}: {error}",
            project_root.display()
        )
    })?;

    match load_requested() {
        Ok(Some(loaded)) => return Ok((loaded, "self-recovered")),
        Ok(None) if has_specialized_request => match load_complete() {
            Ok(Some(loaded)) => return Ok((loaded, "self-recovered")),
            Ok(None) => {}
            Err(error) => {
                return Err(format!(
                    "Hook matcher Binary v1 reload failed after automatic publication for {}: {error}",
                    project_root.display()
                ));
            }
        },
        Ok(None) => {}
        Err(error) => {
            return Err(format!(
                "Hook matcher Binary v1 reload failed after automatic publication for {}: {error}",
                project_root.display()
            ));
        }
    }
    Err(format!(
        "Hook matcher Binary v1 remained unavailable after automatic publication for {}; config={}",
        project_root.display(),
        config_path.display(),
    ))
}

/// Compile and atomically publish one workspace's immutable Hook matcher.
///
/// This is a bounded control-plane operation used by canonical installation and
/// by one in-process recovery attempt when the immutable matcher is absent or
/// corrupt. It does not invoke the Hook CLI or recursively re-enter PreToolUse.
pub(crate) fn publish_hook_matcher_generation(
    config_path: &Path,
    project_root: &Path,
) -> Result<String, String> {
    let generation = compiled_generation_key(config_path, project_root)?;
    let config =
        agent_semantic_hook::load_client_config_for_matcher_publication(config_path, project_root)
            .map_err(|error| {
                format!(
                    "Hook matcher snapshot source is invalid for {}: {error}; {}",
                    config_path.display(),
                    recovery_instruction()
                )
            })?;
    let expected_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    if config.contract_fingerprint() != Some(expected_fingerprint.as_str()) {
        return Err(format!(
            "Hook matcher snapshot source fingerprint mismatch for {}; {}",
            config_path.display(),
            recovery_instruction()
        ));
    }
    agent_semantic_hook::validate_match_policy_rule_coverage(&config).map_err(|error| {
        format!(
            "Hook matcher source conformance failed for {}: {error}; {}",
            config_path.display(),
            recovery_instruction()
        )
    })?;
    publish_active_matcher(config_path, project_root, &generation, &config)?;
    Ok(generation)
}
