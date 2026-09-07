// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! One native ripgrep process per request block over an immutable Runtime corpus.

use std::collections::BTreeSet;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CANDIDATE_LIMIT: usize = 4_096;

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
pub struct ContentBoundNativeRgReceipt {
    pub axis: ContentBoundNativeRgAxisReceipt,
    pub branch_matches: Vec<Vec<ContentBoundNativeRgMatch>>,
    pub process_count: usize,
    pub truncated: bool,
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
            .map_err(|error| format!("spawn content-bound rg: {error}"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "content-bound rg stdin unavailable".to_owned())?;
        let bytes = corpus.bytes.clone();
        let writer = tokio::spawn(async move {
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await
        });
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| "content-bound rg stdout unavailable".to_owned())?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| "content-bound rg stderr unavailable".to_owned())?;
        let mut output = Vec::new();
        let mut error = Vec::new();
        let (_, _, status) = tokio::try_join!(
            stdout.read_to_end(&mut output),
            stderr.read_to_end(&mut error),
            child.wait()
        )
        .map_err(|error| format!("execute content-bound rg: {error}"))?;
        writer
            .await
            .map_err(|error| format!("join content-bound rg writer: {error}"))?
            .map_err(|error| format!("write content-bound rg corpus: {error}"))?;
        if !status.success() && status.code() != Some(1) {
            return Err(format!(
                "content-bound rg failed: {}",
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
                .ok_or_else(|| "content-bound rg omitted line-number evidence".to_owned())?;
            let line = std::str::from_utf8(&line[..separator])
                .map_err(|_| "content-bound rg line number is not UTF-8".to_owned())?
                .parse::<u64>()
                .map_err(|_| "content-bound rg line number is invalid".to_owned())?;
            let owner = crate::owner_for_corpus_line(&corpus.owner_spans, line)
                .ok_or_else(|| "content-bound rg result escaped the immutable corpus".to_owned())?;
            paths.insert(owner.owner_path.clone());
            matches.push(ContentBoundNativeRgMatch {
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
            return Err(format!("rg option is not admitted: {argument}"));
        } else {
            positionals.push(argument.clone());
            index += 1;
        }
    }
    if has_regexp {
        if positionals.iter().any(|value| value != ".") {
            return Err("rg paths must remain the canonical workspace".to_owned());
        }
    } else {
        let pattern = positionals
            .first()
            .ok_or_else(|| "rg block requires a pattern".to_owned())?;
        output.push(pattern.clone());
        if positionals.iter().skip(1).any(|value| value != ".") {
            return Err("rg paths must remain the canonical workspace".to_owned());
        }
    }
    Ok(output)
}

fn axis_receipt(branches: Vec<Vec<String>>) -> ContentBoundNativeRgAxisReceipt {
    let mut candidate_owner_paths = branches.iter().flatten().cloned().collect::<Vec<_>>();
    candidate_owner_paths.sort();
    candidate_owner_paths.dedup();
    candidate_owner_paths.truncate(CANDIDATE_LIMIT);
    ContentBoundNativeRgAxisReceipt {
        branch_candidate_owner_paths: branches,
        candidate_owner_paths,
    }
}
