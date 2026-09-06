//! Runtime-independent Layer One acquisition for Search Playbook.
//!
//! This lane returns workspace owner candidates only. It cannot mint parser
//! selectors, graph relations, or any serving/admission identity.

use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::process::Stdio;

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const LOCAL_SEARCH_PLAYBOOK_ACQUISITION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.local-search-playbook-acquisition-receipt";

const OWNER_LIMIT: usize = 16_384;
const OWNER_BYTE_LIMIT: u64 = 4 * 1024 * 1024;
const CORPUS_BYTE_LIMIT: usize = 128 * 1024 * 1024;
const CANDIDATE_LIMIT: usize = 4_096;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSearchAxisReceipt {
    pub branch_candidate_owner_paths: Vec<Vec<String>>,
    pub candidate_owner_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSearchPlaybookAcquisitionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub content_generation_digest: String,
    pub owner_count: usize,
    pub fd: LocalSearchAxisReceipt,
    pub rg: LocalSearchAxisReceipt,
    pub tantivy: LocalSearchAxisReceipt,
    pub candidate_owner_paths: Vec<String>,
    /// Always empty: only a parser-owned Runtime generation may mint selectors.
    pub exact_selectors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalRgMatch {
    pub owner_path: String,
    pub owner_line: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentBoundNativeRgReceipt {
    pub axis: LocalSearchAxisReceipt,
    pub branch_matches: Vec<Vec<LocalRgMatch>>,
    pub process_count: usize,
    pub truncated: bool,
}

pub(crate) struct WorkspaceOwner {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
    digest: String,
}

pub async fn execute_local_search_playbook_acquisition(
    workspace: &Path,
    fd: &[Vec<String>],
    rg: &[Vec<String>],
    tantivy: &[Vec<String>],
) -> Result<LocalSearchPlaybookAcquisitionReceipt, String> {
    let workspace = std::fs::canonicalize(workspace).map_err(|error| {
        format!("Search Playbook requires a canonical workspace directory: {error}")
    })?;
    if !workspace.is_dir() {
        return Err("Search Playbook canonical workspace is not a directory".to_owned());
    }
    let owners = collect_workspace_owners(&workspace)?;
    let owner_paths = owners
        .iter()
        .map(|owner| owner.path.clone())
        .collect::<Vec<_>>();
    let content_generation_digest = content_generation_digest(&owners);

    let fd = if fd.is_empty() {
        LocalSearchAxisReceipt::default()
    } else {
        let receipt = crate::execute_native_fd_blocks(fd, &owner_paths)?;
        LocalSearchAxisReceipt {
            branch_candidate_owner_paths: receipt.branch_candidate_owner_paths,
            candidate_owner_paths: receipt.candidate_owner_paths,
        }
    };
    let rg = execute_rg_blocks(&owners, rg).await?;
    let tantivy = crate::tantivy_lexical::execute_local_tantivy_blocks(&owners, tantivy)?;

    let mut candidate_owner_paths = fd
        .candidate_owner_paths
        .iter()
        .chain(&rg.candidate_owner_paths)
        .chain(&tantivy.candidate_owner_paths)
        .cloned()
        .collect::<Vec<_>>();
    candidate_owner_paths.sort();
    candidate_owner_paths.dedup();
    candidate_owner_paths.truncate(CANDIDATE_LIMIT);

    Ok(LocalSearchPlaybookAcquisitionReceipt {
        schema_id: LOCAL_SEARCH_PLAYBOOK_ACQUISITION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        content_generation_digest,
        owner_count: owners.len(),
        fd,
        rg,
        tantivy,
        candidate_owner_paths,
        exact_selectors: Vec::new(),
    })
}

fn collect_workspace_owners(workspace: &Path) -> Result<Vec<WorkspaceOwner>, String> {
    let mut owners = Vec::new();
    let mut total_bytes = 0_usize;
    for entry in WalkBuilder::new(workspace)
        .hidden(false)
        .standard_filters(true)
        .build()
    {
        let entry = entry.map_err(|error| format!("walk Search Playbook workspace: {error}"))?;
        let file_type = entry.file_type();
        if !file_type.is_some_and(|file_type| file_type.is_file()) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("inspect Search Playbook owner: {error}"))?;
        if metadata.len() > OWNER_BYTE_LIMIT {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(workspace)
            .map_err(|_| "Search Playbook owner escaped the canonical workspace".to_owned())?;
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("Search Playbook owner path is not normalized".to_owned());
        }
        let path = relative.to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(entry.path())
            .map_err(|error| format!("read Search Playbook owner {path}: {error}"))?;
        if bytes.contains(&0) {
            continue;
        }
        total_bytes = total_bytes.saturating_add(bytes.len());
        if owners.len() == OWNER_LIMIT || total_bytes > CORPUS_BYTE_LIMIT {
            return Err("Search Playbook local acquisition coverage budget exceeded".to_owned());
        }
        let digest = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
        owners.push(WorkspaceOwner {
            path,
            bytes,
            digest,
        });
    }
    owners.sort_by(|left, right| left.path.cmp(&right.path));
    if owners.is_empty() {
        return Err("Search Playbook canonical workspace has no readable owners".to_owned());
    }
    Ok(owners)
}

fn content_generation_digest(owners: &[WorkspaceOwner]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.local-search-playbook-generation.v1\0");
    for owner in owners {
        for value in [owner.path.as_bytes(), owner.digest.as_bytes()] {
            hasher.update(&(value.len() as u64).to_le_bytes());
            hasher.update(value);
        }
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

async fn execute_rg_blocks(
    owners: &[WorkspaceOwner],
    blocks: &[Vec<String>],
) -> Result<LocalSearchAxisReceipt, String> {
    if blocks.is_empty() {
        return Ok(LocalSearchAxisReceipt::default());
    }
    let owner_digests = owners
        .iter()
        .map(|owner| owner.digest.as_str())
        .collect::<Vec<_>>();
    let corpus = crate::build_cold_rg_corpus(
        &content_generation_digest(owners),
        owners
            .iter()
            .zip(owner_digests)
            .map(|(owner, digest)| crate::ColdRgCorpusOwner {
                owner_path: &owner.path,
                content_digest: digest,
                bytes: &owner.bytes,
            }),
    )?;
    Ok(
        execute_content_bound_native_rg_blocks(&corpus, blocks, CANDIDATE_LIMIT)
            .await?
            .axis,
    )
}

pub async fn execute_content_bound_native_rg_blocks(
    corpus: &crate::ColdRgCorpusArtifact,
    blocks: &[Vec<String>],
    limit: usize,
) -> Result<ContentBoundNativeRgReceipt, String> {
    if blocks.is_empty() || limit == 0 || limit > CANDIDATE_LIMIT {
        return Err("content-bound rg request is outside the bounded envelope".to_owned());
    }
    let mut branch_candidate_owner_paths = Vec::with_capacity(blocks.len());
    let mut branch_matches = Vec::with_capacity(blocks.len());
    for block in blocks {
        let argv = native_rg_stdin_argv(block)?;
        let mut child = tokio::process::Command::new("rg")
            .args(argv)
            .args([
                "--line-number",
                "--no-heading",
                "--color=never",
                "--max-count",
            ])
            .arg(limit.to_string())
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("spawn local Search Playbook rg: {error}"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "local rg stdin unavailable".to_owned())?;
        let bytes = corpus.bytes.clone();
        let writer = tokio::spawn(async move {
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await
        });
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| "local rg stdout unavailable".to_owned())?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| "local rg stderr unavailable".to_owned())?;
        let mut output = Vec::new();
        let mut error = Vec::new();
        let (_, _, status) = tokio::try_join!(
            stdout.read_to_end(&mut output),
            stderr.read_to_end(&mut error),
            child.wait()
        )
        .map_err(|error| format!("execute local Search Playbook rg: {error}"))?;
        writer
            .await
            .map_err(|error| format!("join local rg writer: {error}"))?
            .map_err(|error| format!("write local rg corpus: {error}"))?;
        if !status.success() && status.code() != Some(1) {
            return Err(format!(
                "local Search Playbook rg failed: {}",
                String::from_utf8_lossy(&error)
            ));
        }
        let mut paths = BTreeSet::new();
        let mut matches = Vec::new();
        for line in output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let separator = line
                .iter()
                .position(|byte| *byte == b':')
                .ok_or_else(|| "local rg omitted line-number evidence".to_owned())?;
            let line = std::str::from_utf8(&line[..separator])
                .map_err(|_| "local rg line number is not UTF-8".to_owned())?
                .parse::<u64>()
                .map_err(|_| "local rg line number is invalid".to_owned())?;
            let owner = crate::owner_for_corpus_line(&corpus.owner_spans, line)
                .ok_or_else(|| "local rg result escaped the content-bound corpus".to_owned())?;
            paths.insert(owner.owner_path.clone());
            matches.push(LocalRgMatch {
                owner_path: owner.owner_path.clone(),
                owner_line: line - owner.start_line + 1,
            });
            if matches.len() == limit {
                break;
            }
        }
        branch_candidate_owner_paths.push(paths.into_iter().collect());
        branch_matches.push(matches);
    }
    let truncated = branch_matches.iter().any(|matches| matches.len() == limit);
    Ok(ContentBoundNativeRgReceipt {
        axis: axis_receipt(branch_candidate_owner_paths),
        branch_matches,
        process_count: blocks.len(),
        truncated,
    })
}

fn native_rg_stdin_argv(argv: &[String]) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    let mut positionals = Vec::new();
    let mut has_regexp = false;
    let mut index = 0;
    while index < argv.len() {
        let argument = &argv[index];
        if matches!(argument.as_str(), "-e" | "--regexp") {
            let value = argv
                .get(index + 1)
                .ok_or_else(|| "rg --regexp requires a value".to_owned())?;
            output.push(argument.clone());
            output.push(value.clone());
            has_regexp = true;
            index += 2;
        } else if argument.starts_with("--regexp=") {
            output.push(argument.clone());
            has_regexp = true;
            index += 1;
        } else if matches!(
            argument.as_str(),
            "-n" | "--line-number"
                | "-i"
                | "--ignore-case"
                | "-s"
                | "--case-sensitive"
                | "-S"
                | "--smart-case"
                | "-F"
                | "--fixed-strings"
                | "-w"
                | "--word-regexp"
                | "-x"
                | "--line-regexp"
                | "-U"
                | "--multiline"
                | "--multiline-dotall"
                | "--crlf"
        ) {
            output.push(argument.clone());
            index += 1;
        } else if argument.starts_with('-') {
            return Err(format!(
                "rg option is not admitted by local Search Playbook: {argument}"
            ));
        } else {
            positionals.push(argument.clone());
            index += 1;
        }
    }
    if has_regexp {
        if positionals.iter().any(|value| value != ".") {
            return Err("local rg paths must remain the canonical workspace".to_owned());
        }
    } else {
        let pattern = positionals
            .first()
            .ok_or_else(|| "rg block requires a pattern".to_owned())?;
        output.push(pattern.clone());
        if positionals.iter().skip(1).any(|value| value != ".") {
            return Err("local rg paths must remain the canonical workspace".to_owned());
        }
    }
    Ok(output)
}

pub(crate) fn axis_receipt(branches: Vec<Vec<String>>) -> LocalSearchAxisReceipt {
    let mut candidate_owner_paths = branches.iter().flatten().cloned().collect::<Vec<_>>();
    candidate_owner_paths.sort();
    candidate_owner_paths.dedup();
    candidate_owner_paths.truncate(CANDIDATE_LIMIT);
    LocalSearchAxisReceipt {
        branch_candidate_owner_paths: branches,
        candidate_owner_paths,
    }
}

pub(crate) fn owner_paths(owners: &[WorkspaceOwner]) -> Vec<String> {
    owners.iter().map(|owner| owner.path.clone()).collect()
}

pub(crate) fn owner_terms(owners: &[WorkspaceOwner]) -> Vec<Vec<String>> {
    owners
        .iter()
        .map(|owner| {
            crate::source_index_lookup_terms(&String::from_utf8_lossy(&owner.bytes))
                .into_iter()
                .filter(|term| !term.chars().any(char::is_whitespace))
                .collect()
        })
        .collect()
}
