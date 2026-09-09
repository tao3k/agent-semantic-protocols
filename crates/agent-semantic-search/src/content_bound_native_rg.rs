// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Exact native `rg` execution over an ephemeral immutable-generation tree.

use std::collections::BTreeSet;
use std::io::Read;
use std::process::{Command, Stdio};

use agent_semantic_shell_parser::{NativeRgArgvAnalysis, NativeRgOutputAttribution};
use serde::{Deserialize, Serialize};

const MAX_NATIVE_RG_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentBoundNativeRgAxisReceipt {
    pub branch_candidate_owner_paths: Vec<Vec<String>>,
    pub candidate_owner_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentBoundNativeRgMatch {
    pub owner_path: String,
    pub owner_line: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentBoundNativeRgProcessReceipt {
    pub exact_argv: Vec<String>,
    pub argv_digest: String,
    pub exit_code: i32,
    pub stdout_byte_count: usize,
    pub stderr_byte_count: usize,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub output_attribution: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentBoundNativeRgReceipt {
    pub axis: ContentBoundNativeRgAxisReceipt,
    pub branch_matches: Vec<Vec<ContentBoundNativeRgMatch>>,
    pub processes: Vec<ContentBoundNativeRgProcessReceipt>,
    pub process_count: usize,
    pub truncated: bool,
}

pub fn execute_content_bound_native_rg_blocks(
    corpus: &crate::ColdRgCorpusArtifact,
    blocks: &[Vec<String>],
    limit: usize,
) -> Result<ContentBoundNativeRgReceipt, String> {
    if blocks.is_empty() || limit == 0 {
        return Err("content-bound rg request is outside the bounded envelope".to_owned());
    }
    validate_corpus_receipt(corpus)?;
    let snapshot = tempfile::Builder::new()
        .prefix("asp-native-rg-")
        .tempdir()
        .map_err(|error| format!("create immutable rg snapshot root: {error}"))?;
    materialize_generation_tree(corpus, snapshot.path())?;

    let mut branch_candidate_owner_paths = Vec::with_capacity(blocks.len());
    let mut branch_matches = Vec::with_capacity(blocks.len());
    let mut processes = Vec::with_capacity(blocks.len());
    let mut truncated = false;
    for (block_index, block) in blocks.iter().enumerate() {
        let analysis = agent_semantic_shell_parser::analyze_native_rg_argv(block);
        if !analysis.is_admitted() {
            return Err(format!(
                "native rg argv is not admitted: blockIndex={block_index} diagnostics={:?}",
                analysis.diagnostics
            ));
        }
        let mut child = Command::new("rg")
            .args(block)
            .current_dir(snapshot.path())
            .env_remove("RIPGREP_CONFIG_PATH")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("execute admitted native rg: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "native rg stdout pipe was not captured".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "native rg stderr pipe was not captured".to_owned())?;
        let stdout_reader = std::thread::spawn(move || bounded_capture(stdout));
        let stderr_reader = std::thread::spawn(move || bounded_capture(stderr));
        let status = child
            .wait()
            .map_err(|error| format!("wait for admitted native rg: {error}"))?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| "native rg stdout capture panicked".to_owned())??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| "native rg stderr capture panicked".to_owned())??;
        if stdout.exceeded || stderr.exceeded {
            return Err(format!(
                "native rg output exceeded the bounded envelope: blockIndex={block_index} stdoutBytes={} stderrBytes={} maximumBytes={MAX_NATIVE_RG_OUTPUT_BYTES}",
                stdout.byte_count, stderr.byte_count
            ));
        }
        let exit_code = status.code().ok_or_else(|| {
            format!("native rg terminated without an exit code: blockIndex={block_index}")
        })?;
        let process_receipt = ContentBoundNativeRgProcessReceipt {
            exact_argv: block.clone(),
            argv_digest: digest(
                &serde_json::to_vec(block)
                    .map_err(|error| format!("encode exact native rg argv: {error}"))?,
            ),
            exit_code,
            stdout_byte_count: stdout.byte_count,
            stderr_byte_count: stderr.byte_count,
            stdout_digest: stdout.digest,
            stderr_digest: stderr.digest,
            output_attribution: analysis.output_attribution.as_str().to_owned(),
        };
        if !matches!(exit_code, 0 | 1) {
            return Err(format!(
                "native rg failed: blockIndex={block_index} exitCode={exit_code} stdoutDigest={} stderrDigest={} stderr={}",
                process_receipt.stdout_digest,
                process_receipt.stderr_digest,
                String::from_utf8_lossy(&stderr.bytes).trim()
            ));
        }
        let decoded = if exit_code == 1 {
            DecodedRgOutput::default()
        } else {
            decode_native_rg_output(
                &stdout.bytes,
                &analysis,
                snapshot.path(),
                &corpus.owner_spans,
                limit,
            )?
        };
        if exit_code == 0 && !stdout.bytes.is_empty() && decoded.owner_paths.is_empty() {
            return Err(format!(
                "reasonKind=rg-output-not-attributable blockIndex={block_index} stdoutDigest={} outputAttribution={}",
                process_receipt.stdout_digest,
                analysis.output_attribution.as_str()
            ));
        }
        truncated |= decoded.truncated;
        branch_candidate_owner_paths.push(decoded.owner_paths);
        branch_matches.push(decoded.matches);
        processes.push(process_receipt);
    }
    Ok(ContentBoundNativeRgReceipt {
        axis: axis_receipt(branch_candidate_owner_paths),
        branch_matches,
        process_count: processes.len(),
        processes,
        truncated,
    })
}

struct BoundedCapture {
    bytes: Vec<u8>,
    byte_count: usize,
    digest: String,
    exceeded: bool,
}

fn bounded_capture(mut reader: impl Read) -> Result<BoundedCapture, String> {
    let mut retained = Vec::with_capacity(64 * 1024);
    let mut buffer = [0_u8; 64 * 1024];
    let mut byte_count = 0_usize;
    let mut hasher = blake3::Hasher::new();
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("capture native rg output: {error}"))?;
        if read == 0 {
            break;
        }
        byte_count = byte_count.saturating_add(read);
        hasher.update(&buffer[..read]);
        let remaining = MAX_NATIVE_RG_OUTPUT_BYTES.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(remaining)]);
    }
    Ok(BoundedCapture {
        bytes: retained,
        byte_count,
        digest: format!("blake3-256:{}", hasher.finalize().to_hex()),
        exceeded: byte_count > MAX_NATIVE_RG_OUTPUT_BYTES,
    })
}

fn validate_corpus_receipt(corpus: &crate::ColdRgCorpusArtifact) -> Result<(), String> {
    if corpus.receipt.owner_count != corpus.owner_spans.len()
        || digest(&corpus.bytes) != corpus.receipt.corpus_digest
        || digest(
            &serde_json::to_vec(&corpus.owner_spans)
                .map_err(|error| format!("encode cold rg owner spans: {error}"))?,
        ) != corpus.receipt.owner_spans_digest
    {
        return Err("cold rg corpus receipt does not bind the provided artifact".to_owned());
    }
    Ok(())
}

fn materialize_generation_tree(
    corpus: &crate::ColdRgCorpusArtifact,
    root: &std::path::Path,
) -> Result<(), String> {
    let mut cursor = 0;
    let mut expected_start_line = 1_u64;
    for owner in &corpus.owner_spans {
        if owner.start_line != expected_start_line || owner.end_line < owner.start_line {
            return Err("cold rg owner spans are not contiguous and canonical".to_owned());
        }
        let start = cursor;
        for _ in owner.start_line..=owner.end_line {
            let relative = corpus.bytes[cursor..]
                .iter()
                .position(|byte| *byte == b'\n')
                .ok_or_else(|| "cold rg owner span exceeds corpus bytes".to_owned())?;
            cursor += relative + 1;
        }
        let normalized = &corpus.bytes[start..cursor];
        let bytes = if digest(normalized) == owner.content_digest {
            normalized
        } else if normalized.ends_with(b"\n")
            && digest(&normalized[..normalized.len() - 1]) == owner.content_digest
        {
            &normalized[..normalized.len() - 1]
        } else {
            return Err(format!(
                "cold rg owner span does not reconstruct its content digest: {}",
                owner.owner_path
            ));
        };
        let destination = root.join(&owner.owner_path);
        let parent = destination
            .parent()
            .ok_or_else(|| "cold rg owner has no snapshot parent".to_owned())?;
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create immutable rg snapshot directory {}: {error}",
                parent.display()
            )
        })?;
        std::fs::write(&destination, bytes).map_err(|error| {
            format!(
                "write immutable rg snapshot owner {}: {error}",
                destination.display()
            )
        })?;
        let mut permissions = std::fs::metadata(&destination)
            .map_err(|error| format!("read immutable rg snapshot permissions: {error}"))?
            .permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&destination, permissions)
            .map_err(|error| format!("seal immutable rg snapshot owner: {error}"))?;
        expected_start_line = owner.end_line + 1;
    }
    if cursor != corpus.bytes.len() {
        return Err("cold rg owner spans do not consume the complete corpus".to_owned());
    }
    Ok(())
}

#[derive(Default)]
struct DecodedRgOutput {
    owner_paths: Vec<String>,
    matches: Vec<ContentBoundNativeRgMatch>,
    truncated: bool,
}

fn decode_native_rg_output(
    stdout: &[u8],
    analysis: &NativeRgArgvAnalysis,
    snapshot_root: &std::path::Path,
    owner_spans: &[crate::ColdRgOwnerSpan],
    limit: usize,
) -> Result<DecodedRgOutput, String> {
    let aliases = owner_aliases(snapshot_root, owner_spans);
    if analysis.output_attribution == NativeRgOutputAttribution::JsonPathLine {
        decode_json_output(stdout, &aliases, limit)
    } else {
        decode_text_output(stdout, analysis, &aliases, limit)
    }
}

fn owner_aliases(
    snapshot_root: &std::path::Path,
    owner_spans: &[crate::ColdRgOwnerSpan],
) -> Vec<(String, String)> {
    let mut aliases = owner_spans
        .iter()
        .flat_map(|owner| {
            let absolute = snapshot_root.join(&owner.owner_path);
            [
                (owner.owner_path.clone(), owner.owner_path.clone()),
                (format!("./{}", owner.owner_path), owner.owner_path.clone()),
                (absolute.display().to_string(), owner.owner_path.clone()),
            ]
        })
        .collect::<Vec<_>>();
    aliases.sort_by(|left, right| {
        right
            .0
            .len()
            .cmp(&left.0.len())
            .then_with(|| left.cmp(right))
    });
    aliases.dedup();
    aliases
}

fn decode_json_output(
    stdout: &[u8],
    aliases: &[(String, String)],
    limit: usize,
) -> Result<DecodedRgOutput, String> {
    let mut decoded = DecodedRgOutput::default();
    let mut owners = BTreeSet::new();
    for line in stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: serde_json::Value = serde_json::from_slice(line)
            .map_err(|error| format!("decode native rg JSON output: {error}"))?;
        if value.get("type").and_then(serde_json::Value::as_str) != Some("match") {
            continue;
        }
        let path = value
            .pointer("/data/path/text")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "native rg JSON match omitted a UTF-8 path".to_owned())?;
        let owner = canonical_owner(path, aliases)
            .ok_or_else(|| format!("native rg JSON path escaped the immutable corpus: {path}"))?;
        owners.insert(owner.clone());
        if let Some(owner_line) = value
            .pointer("/data/line_number")
            .and_then(serde_json::Value::as_u64)
        {
            decoded.matches.push(ContentBoundNativeRgMatch {
                owner_path: owner,
                owner_line,
            });
        }
        if owners.len().max(decoded.matches.len()) > limit {
            decoded.truncated = true;
            break;
        }
    }
    decoded.owner_paths = owners.into_iter().take(limit).collect();
    decoded.matches.truncate(limit);
    Ok(decoded)
}

fn decode_text_output(
    stdout: &[u8],
    analysis: &NativeRgArgvAnalysis,
    aliases: &[(String, String)],
    limit: usize,
) -> Result<DecodedRgOutput, String> {
    let mut decoded = DecodedRgOutput::default();
    let mut owners = BTreeSet::new();
    let mut current_heading = None::<String>;
    let path_only = analysis.output_attribution == NativeRgOutputAttribution::PathOnly;
    let normalized = stdout
        .iter()
        .map(|byte| if *byte == 0 { b':' } else { *byte })
        .collect::<Vec<_>>();
    for raw_line in normalized
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let line = String::from_utf8_lossy(raw_line);
        if let Some(owner) = canonical_owner(line.as_ref(), aliases) {
            owners.insert(owner.clone());
            current_heading = Some(owner);
        } else if let Some((owner, remainder)) = owner_prefixed_remainder(line.as_ref(), aliases) {
            owners.insert(owner.clone());
            current_heading = Some(owner.clone());
            if !path_only && let Some(owner_line) = leading_delimited_number(remainder) {
                decoded.matches.push(ContentBoundNativeRgMatch {
                    owner_path: owner,
                    owner_line,
                });
            }
        } else if !path_only
            && let Some(owner) = current_heading.clone()
            && let Some(owner_line) = leading_number(line.as_ref())
        {
            decoded.matches.push(ContentBoundNativeRgMatch {
                owner_path: owner,
                owner_line,
            });
        }
        if owners.len().max(decoded.matches.len()) > limit {
            decoded.truncated = true;
            break;
        }
    }
    decoded.owner_paths = owners.into_iter().take(limit).collect();
    decoded.matches.sort_by(|left, right| {
        left.owner_path
            .cmp(&right.owner_path)
            .then_with(|| left.owner_line.cmp(&right.owner_line))
    });
    decoded.matches.dedup();
    decoded.matches.truncate(limit);
    Ok(decoded)
}

fn canonical_owner(value: &str, aliases: &[(String, String)]) -> Option<String> {
    let value = value.trim_end_matches(['\r', ':']);
    aliases
        .iter()
        .find(|(alias, _)| value == alias)
        .map(|(_, owner)| owner.clone())
}

fn owner_prefixed_remainder<'a>(
    value: &'a str,
    aliases: &[(String, String)],
) -> Option<(String, &'a str)> {
    aliases.iter().find_map(|(alias, owner)| {
        value
            .strip_prefix(alias)
            .filter(|remainder| {
                remainder
                    .chars()
                    .next()
                    .is_some_and(|character| !character.is_alphanumeric())
            })
            .map(|remainder| (owner.clone(), remainder))
    })
}

fn leading_delimited_number(value: &str) -> Option<u64> {
    let value = value.strip_prefix(':')?;
    leading_number(value)
}

fn leading_number(value: &str) -> Option<u64> {
    let digits = value
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

fn axis_receipt(branches: Vec<Vec<String>>) -> ContentBoundNativeRgAxisReceipt {
    let mut candidate_owner_paths = branches.iter().flatten().cloned().collect::<Vec<_>>();
    candidate_owner_paths.sort();
    candidate_owner_paths.dedup();
    ContentBoundNativeRgAxisReceipt {
        branch_candidate_owner_paths: branches,
        candidate_owner_paths,
    }
}
