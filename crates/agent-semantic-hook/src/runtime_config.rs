//! Immutable, process-independent Hook matcher snapshot publication and loading.

use crate::{ClientHookConfig, DecisionKind, DurableHookConfigArtifact, HookDecision};
use memmap2::MmapOptions;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

const ACTIVE_MATCHER_ARTIFACT: &str = "matcher.bin";
const ACTIVE_MATCHER_MAGIC: &[u8; 8] = b"ASPHK1PC";
const ACTIVE_MATCHER_HEADER_LEN: usize = 8 + 8 + 16 + 8 + 16 + 64;
const ACTIVE_MATCHER_SECTION_ENTRY_LEN: usize = 1 + 8 + 8 + 8 + 32;

#[cfg(test)]
#[path = "../tests/unit/runtime_config.rs"]
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
    content_digest_prefix: u128,
}

fn source_stamp(path: &Path) -> Result<SourceStamp, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(SourceStamp {
            byte_len: u64::try_from(bytes.len())
                .map_err(|_| format!("Hook matcher source is too large: {}", path.display()))?,
            content_digest_prefix: u128::from_le_bytes(
                blake3::hash(&bytes).as_bytes()[..16]
                    .try_into()
                    .expect("BLAKE3 prefix is sixteen bytes"),
            ),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SourceStamp {
            byte_len: 0,
            content_digest_prefix: 0,
        }),
        Err(error) => Err(format!(
            "stat Hook matcher source {}: {error}",
            path.display()
        )),
    }
}

pub fn hook_state_home() -> Result<PathBuf, String> {
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

fn active_generation_root() -> Result<PathBuf, String> {
    let root = match std::env::var_os("ASP_HOOK_GENERATION_ROOT") {
        Some(root) if !root.is_empty() => PathBuf::from(root),
        Some(_) => return Err("ASP_HOOK_GENERATION_ROOT is set but empty".to_owned()),
        None => fs::canonicalize(hook_state_home()?.join("hooks/current"))
            .map_err(|error| format!("resolve HookGeneration current: {error}"))?,
    };
    Ok(root)
}

fn active_generation_digest() -> Result<String, String> {
    let root = active_generation_root()?;
    let digest = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "HookGeneration root has no digest component".to_owned())?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "HookGeneration root digest is invalid: {}",
            root.display()
        ));
    }
    Ok(format!("blake3-256:{digest}"))
}

fn active_matcher_path(_project_root: &Path) -> Result<PathBuf, String> {
    Ok(active_generation_root()?
        .join("compiled")
        .join(ACTIVE_MATCHER_ARTIFACT))
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
        source_stamp(&crate::project_agent_config_path(project_root))?,
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
    shell_read_keys: &[crate::ShellReadSourceKey],
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
    let published_config_digest_prefix = take_u128(&mapped, &mut offset)?;
    let published_agents_byte_len = take_u64(&mapped, &mut offset)?;
    let published_agents_digest_prefix = take_u128(&mapped, &mut offset)?;
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
        published_config_digest_prefix,
        published_agents_byte_len,
        published_agents_digest_prefix,
    );
    let bundle = &mapped[offset..];
    let mut shell_read_allow: Option<HookDecision> = None;
    for key in shell_read_keys {
        if let Some(table) = select_matcher_section(
            bundle,
            MatcherSectionKind::ShellRead,
            matcher_section_key(&key.extension),
        )? {
            if let Some(mut decision) = crate::CommandDecisionShard::select_for_command(
                table,
                &key.command,
                &key.command_tokens,
            )? {
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
            let mut decision = crate::HookDecision::from_compact_binary(template)?;
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

fn compile_active_matcher(
    config_path: &Path,
    project_root: &Path,
    generation: &str,
    compiled: &ClientHookConfig,
) -> Result<Vec<u8>, String> {
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
    let mut bytes = Vec::with_capacity(ACTIVE_MATCHER_HEADER_LEN + bundle.len());
    bytes.extend_from_slice(ACTIVE_MATCHER_MAGIC);
    bytes.extend_from_slice(&config.byte_len.to_le_bytes());
    bytes.extend_from_slice(&config.content_digest_prefix.to_le_bytes());
    bytes.extend_from_slice(&agents.byte_len.to_le_bytes());
    bytes.extend_from_slice(&agents.content_digest_prefix.to_le_bytes());
    bytes.extend_from_slice(generation.as_bytes());
    bytes.extend_from_slice(&bundle);
    Ok(bytes)
}

pub struct LoadedHookConfig {
    pub config: Option<ClientHookConfig>,
    pub decision: Option<HookDecision>,
    pub projection: Option<&'static str>,
}

fn recovery_instruction() -> &'static str {
    "repair invalid source config or atomically publish a validated HookGeneration with `<candidate-asp> install binary`; PreTool never compiles or publishes policy"
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
        &crate::hook_runtime_artifact_fingerprint(),
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
    hasher.update(agent_semantic_config::hook_client_contract_fingerprint().as_bytes());
    hasher.update(artifact_fingerprint.as_bytes());
    append_file_identity(&mut hasher, config_path)?;
    append_file_identity(&mut hasher, &crate::project_agent_config_path(project_root))?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn load_fresh_hook_config(
    _config_path: &Path,
    project_root: &Path,
    direct_read_extension: Option<&str>,
    direct_read_path: Option<&str>,
    shell_read_keys: &[crate::ShellReadSourceKey],
    shell_command_keys: &[crate::ShellCommandKey],
) -> Result<(LoadedHookConfig, String), String> {
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
        Ok(Some(loaded)) => return Ok((loaded, active_generation_digest()?)),
        Ok(None) if has_specialized_request => match load_complete() {
            Ok(Some(loaded)) => Ok((loaded, active_generation_digest()?)),
            Ok(None) => Err(format!(
                "HookGeneration current omitted its compiled matcher for {}; {}",
                project_root.display(),
                recovery_instruction()
            )),
            Err(error) => Err(format!(
                "HookGeneration current is unavailable for {}: {error}; {}",
                project_root.display(),
                recovery_instruction()
            )),
        },
        Ok(None) => Err(format!(
            "HookGeneration current omitted its compiled matcher for {}; {}",
            project_root.display(),
            recovery_instruction()
        )),
        Err(error) => Err(format!(
            "HookGeneration current is unavailable for {}: {error}; {}",
            project_root.display(),
            recovery_instruction()
        )),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledHookMatcherGeneration {
    pub compiler_generation: String,
    pub bytes: Vec<u8>,
}

/// Compile one immutable matcher candidate without mutating active Hook state.
pub fn compile_hook_matcher_generation(
    config_path: &Path,
    project_root: &Path,
) -> Result<CompiledHookMatcherGeneration, String> {
    let generation = compiled_generation_key(config_path, project_root)?;
    let config = crate::load_client_config_for_matcher_publication(config_path, project_root)
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
    crate::validate_match_policy_rule_coverage(&config).map_err(|error| {
        format!(
            "Hook matcher source conformance failed for {}: {error}; {}",
            config_path.display(),
            recovery_instruction()
        )
    })?;
    let bytes = compile_active_matcher(config_path, project_root, &generation, &config)?;
    validate_compiled_hook_matcher(&bytes)?;
    Ok(CompiledHookMatcherGeneration {
        compiler_generation: generation,
        bytes,
    })
}

/// Validate the complete matcher projection before Artifacts may commit a
/// HookGeneration current pointer.
pub fn validate_compiled_hook_matcher(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < ACTIVE_MATCHER_HEADER_LEN
        || bytes.get(..8) != Some(ACTIVE_MATCHER_MAGIC.as_slice())
    {
        return Err("Hook matcher Binary v1 candidate header is invalid".to_owned());
    }
    let generation = bytes
        .get(ACTIVE_MATCHER_HEADER_LEN - 64..ACTIVE_MATCHER_HEADER_LEN)
        .ok_or_else(|| "Hook matcher Binary v1 candidate generation is truncated".to_owned())?;
    if !generation.iter().all(u8::is_ascii_hexdigit) {
        return Err("Hook matcher Binary v1 candidate generation is invalid".to_owned());
    }
    let bundle = &bytes[ACTIVE_MATCHER_HEADER_LEN..];
    let complete = select_matcher_section(bundle, MatcherSectionKind::Complete, [0; 8])?
        .ok_or_else(|| "Hook matcher Binary v1 candidate omitted complete projection".to_owned())?;
    let artifact = DurableHookConfigArtifact::from_binary_bytes(complete)
        .map_err(|error| format!("decode Hook matcher Binary v1 candidate: {error}"))?;
    ClientHookConfig::from_durable_snapshot_config(artifact)
        .map(|_| ())
        .map_err(|error| format!("hydrate Hook matcher Binary v1 candidate: {error}"))
}
