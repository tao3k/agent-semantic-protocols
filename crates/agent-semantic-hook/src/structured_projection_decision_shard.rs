//! Compact config-derived policy shard for structured document projections.

use serde::Deserialize;
use serde::Serialize;

use crate::HookDecision;
use crate::ShellCommandKey;

#[derive(Serialize, Deserialize)]
struct StructuredProjectionDecisionEntry {
    projection: agent_semantic_config::HookClientStructuredProjectionMatchConfig,
    placeholder: String,
    bounded_decision: Vec<u8>,
    rejected_decision: Vec<u8>,
}

/// Immutable structured-projection policy selected without hydrating the
/// complete Hook matcher.
pub struct StructuredProjectionDecisionShard {
    entries: Vec<StructuredProjectionDecisionEntry>,
}

const STRUCTURED_PROJECTION_SHARD_MAGIC: &[u8; 8] = b"ASPSTPJ1";

impl StructuredProjectionDecisionShard {
    /// Compile config-owned projector grammars and their complete-policy
    /// winners into one mmap section.
    pub fn new(
        entries: Vec<(
            agent_semantic_config::HookClientStructuredProjectionMatchConfig,
            String,
            HookDecision,
            HookDecision,
        )>,
    ) -> Result<Self, String> {
        entries
            .into_iter()
            .map(
                |(projection, placeholder, bounded_decision, rejected_decision)| {
                    Ok(StructuredProjectionDecisionEntry {
                        projection,
                        placeholder,
                        bounded_decision: bounded_decision.to_compact_binary()?,
                        rejected_decision: rejected_decision.to_compact_binary()?,
                    })
                },
            )
            .collect::<Result<Vec<_>, String>>()
            .map(|entries| Self { entries })
    }

    /// Encode the compact table for atomic mmap publication.
    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(STRUCTURED_PROJECTION_SHARD_MAGIC);
        push_u32(&mut bytes, self.entries.len(), "entry count")?;
        for entry in &self.entries {
            let binary = entry.projection.binary.as_bytes();
            push_u32(&mut bytes, binary.len(), "binary length")?;
            bytes.extend_from_slice(binary);
            let record = postcard::to_allocvec(entry).map_err(|error| {
                format!("encode structured-projection decision record: {error}")
            })?;
            push_u32(&mut bytes, record.len(), "record length")?;
            bytes.extend_from_slice(&record);
        }
        Ok(bytes)
    }

    /// Select a pre-admitted decision for one normalized shell action. A
    /// command which names none of the configured projector binaries is not
    /// applicable and must continue through the next policy shard.
    pub fn select(bytes: &[u8], key: &ShellCommandKey) -> Result<Option<HookDecision>, String> {
        let mut cursor = bytes;
        let magic = take_bytes(&mut cursor, STRUCTURED_PROJECTION_SHARD_MAGIC.len())?;
        if magic != STRUCTURED_PROJECTION_SHARD_MAGIC {
            return Err("structured-projection decision shard version mismatch".to_owned());
        }
        let entry_count = take_u32(&mut cursor, "entry count")?;
        for _ in 0..entry_count {
            let binary_length = take_u32(&mut cursor, "binary length")?;
            let binary = std::str::from_utf8(take_bytes(&mut cursor, binary_length)?)
                .map_err(|error| format!("structured projector binary is not UTF-8: {error}"))?;
            let record_length = take_u32(&mut cursor, "record length")?;
            let record = take_bytes(&mut cursor, record_length)?;
            // `ShellCommandKey` tokens are already produced by the shared Bash
            // parser.  Use that typed argv projection to select the configured
            // projector before invoking its richer bounded-slice classifier;
            // otherwise every unrelated projector reparses the same command
            // on the sub-millisecond Hook path.
            if !command_invokes_binary(&key.command_tokens, binary) {
                continue;
            }
            let entry = postcard::from_bytes::<StructuredProjectionDecisionEntry>(record).map_err(
                |error| format!("decode structured-projection decision record: {error}"),
            )?;
            let classification =
                agent_semantic_shell_parser::structured::classify_single_bounded_path_tokens(
                    &key.command_tokens,
                    agent_semantic_shell_parser::structured::BoundedPathCommandSpec {
                        binary: &entry.projection.binary,
                        optional_subcommand_any: &entry.projection.optional_subcommand_any,
                        option_any: &entry.projection.option_any,
                        option_value_arity: &entry.projection.option_value_arity,
                        max_slice_items: entry.projection.max_slice_items,
                    },
                );
            if matches!(
                classification,
        agent_semantic_shell_parser::structured::StructuredFilterClassification::NotApplicable
            ) {
                continue;
            }
            let bounded = matches!(
                &classification,
        agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedPath { .. }
        | agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedScalarPredicate { .. }
            );
            let bounded_source = match &classification {
            agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedPath {
                    source_operands,
                    ..
                }
            | agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedScalarPredicate {
                    source_operands,
                    ..
                } => source_operands.first().map(String::as_str),
                _ => None,
            };
            let structured_source = bounded_source.or_else(|| {
                key.paths
                    .iter()
                    .find(|path| document_format_matches(&entry.projection.document_format, path))
                    .map(String::as_str)
            });
            let Some(source) = structured_source else {
                continue;
            };
            let bounded = bounded
                && document_format_matches(&entry.projection.document_format, &source)
                && std::path::Path::new(&source).is_file();
            let template = if bounded {
                &entry.bounded_decision
            } else {
                &entry.rejected_decision
            };
            let mut decision = HookDecision::from_compact_binary(template)?;
            // The caller always rebinds shell decisions from the original
            // Host payload.  Drop template subject text before marker walking
            // and avoid cloning it here; the exact command/tool authority is
            // restored once, after shard selection.
            decision.subject.command = None;
            decision.subject.tool_name = None;
            if !decision.replace_template_marker(&entry.placeholder, &source) {
                return Err(
                    "structured-projection decision shard omitted its source placeholder"
                        .to_owned(),
                );
            }
            return Ok(Some(decision));
        }
        Ok(None)
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: usize, label: &str) -> Result<(), String> {
    let value = u32::try_from(value)
        .map_err(|_| format!("structured-projection shard {label} exceeds u32"))?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn take_u32(cursor: &mut &[u8], label: &str) -> Result<usize, String> {
    let raw = take_bytes(cursor, std::mem::size_of::<u32>())?;
    let value = u32::from_le_bytes(
        raw.try_into()
            .expect("four-byte structured-projection shard integer"),
    );
    usize::try_from(value).map_err(|_| format!("structured-projection shard {label} exceeds usize"))
}

fn take_bytes<'a>(cursor: &mut &'a [u8], length: usize) -> Result<&'a [u8], String> {
    if cursor.len() < length {
        return Err("structured-projection decision shard is truncated".to_owned());
    }
    let (value, remaining) = cursor.split_at(length);
    *cursor = remaining;
    Ok(value)
}

fn command_invokes_binary(tokens: &[String], binary: &str) -> bool {
    tokens.iter().any(|token| {
        token == binary
            || std::path::Path::new(token)
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                == Some(binary)
    })
}

fn document_format_matches(
    format: &agent_semantic_config::HookClientStructuredFormat,
    path: &str,
) -> bool {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str);
    matches!(
        (format, extension),
        (
            agent_semantic_config::HookClientStructuredFormat::Json,
            Some("json")
        ) | (
            agent_semantic_config::HookClientStructuredFormat::Toml,
            Some("toml")
        )
    )
}
