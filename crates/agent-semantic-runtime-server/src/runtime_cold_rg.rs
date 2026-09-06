//! One bounded ripgrep process over an immutable content-generation corpus.

use std::collections::BTreeSet;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

const MAX_RG_LINE_BYTES: usize = 4096;
const MAX_NATIVE_RG_MATCHES: u32 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeColdRgReceipt {
    pub content_generation_digest: String,
    pub coverage_input_digest: String,
    pub candidate_owner_paths: Vec<String>,
    pub elapsed_micros: u64,
    pub process_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeNativeRgAxisReceipt {
    pub content_generation_digest: String,
    pub candidate_owner_paths: Vec<String>,
    pub branch_candidate_owner_paths: Vec<Vec<String>>,
    pub branch_matches: Vec<Vec<RuntimeRgMatch>>,
    pub process_count: u8,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeRgMatch {
    pub owner_path: String,
    pub owner_line: u64,
}

pub(crate) async fn execute_runtime_native_rg_blocks(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    blocks: &[Vec<String>],
    limit: u32,
    deadline: Duration,
) -> Result<RuntimeNativeRgAxisReceipt, String> {
    if blocks.is_empty() || !(1..=MAX_NATIVE_RG_MATCHES).contains(&limit) || deadline.is_zero() {
        return Err("native rg axis is outside the bounded Runtime envelope".to_owned());
    }
    let receipt = tokio::time::timeout(
        deadline,
        agent_semantic_search::execute_content_bound_native_rg_blocks(
            corpus,
            blocks,
            limit as usize,
        ),
    )
    .await
    .map_err(|_| "native rg axis deadline exceeded; process capability dropped".to_owned())??;
    Ok(RuntimeNativeRgAxisReceipt {
        content_generation_digest: corpus.receipt.content_generation_digest.clone(),
        candidate_owner_paths: receipt.axis.candidate_owner_paths,
        branch_candidate_owner_paths: receipt.axis.branch_candidate_owner_paths,
        branch_matches: receipt
            .branch_matches
            .into_iter()
            .map(|branch| {
                branch
                    .into_iter()
                    .map(|item| RuntimeRgMatch {
                        owner_path: item.owner_path,
                        owner_line: item.owner_line,
                    })
                    .collect()
            })
            .collect(),
        process_count: u8::try_from(receipt.process_count).unwrap_or(u8::MAX),
        truncated: receipt.truncated,
    })
}

pub(crate) async fn execute_runtime_cold_rg(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    query: &str,
    limit: u32,
    deadline: Duration,
) -> Result<RuntimeColdRgReceipt, String> {
    if query.trim().is_empty() || !(1..=100).contains(&limit) || deadline.is_zero() {
        return Err("cold rg request is outside the bounded Runtime envelope".to_owned());
    }
    let terms = agent_semantic_search::source_index_lookup_terms(query)
        .into_iter()
        .filter(|term| !term.trim().is_empty())
        .collect::<BTreeSet<_>>();
    if terms.is_empty() {
        return Err("cold rg request has no normalized lexical terms".to_owned());
    }
    let coverage_input_digest = coverage_input_digest(corpus, &terms, limit);
    let started = tokio::time::Instant::now();
    let mut command = tokio::process::Command::new("rg");
    command
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .args([
            "--line-number",
            "--no-heading",
            "--color=never",
            "--fixed-strings",
            "--max-columns",
        ])
        .arg(MAX_RG_LINE_BYTES.to_string())
        .arg("--max-count")
        .arg(limit.to_string());
    for term in &terms {
        command.arg("-e").arg(term);
    }
    command.arg("-");
    let mut child = command
        .spawn()
        .map_err(|error| format!("spawn bounded cold rg: {error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "cold rg stdin pipe is unavailable".to_owned())?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "cold rg stdout pipe is unavailable".to_owned())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "cold rg stderr pipe is unavailable".to_owned())?;
    let execution = async {
        let write = async {
            stdin.write_all(&corpus.bytes).await?;
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<(), std::io::Error>(())
        };
        let read_stdout = async {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).await.map(|_| bytes)
        };
        let read_stderr = async {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.map(|_| bytes)
        };
        let ((), output, error, status) =
            tokio::try_join!(write, read_stdout, read_stderr, child.wait())?;
        Ok::<_, std::io::Error>((status, output, error))
    };
    let (status, output, error) = match tokio::time::timeout(deadline, execution).await {
        Ok(result) => result.map_err(|error| format!("execute bounded cold rg: {error}"))?,
        Err(_) => {
            let kill = child.kill().await;
            let reap = child.wait().await;
            return Err(format!(
                "cold rg deadline exceeded; process tree killed and reaped (kill={kill:?}, reap={reap:?})"
            ));
        }
    };
    if !status.success() && status.code() != Some(1) {
        return Err(format!(
            "cold rg failed: status={status} stderr={}",
            String::from_utf8_lossy(&error)
        ));
    }
    let candidate_owner_paths = owner_paths_from_rg_output(corpus, &output, limit as usize)?;
    Ok(RuntimeColdRgReceipt {
        content_generation_digest: corpus.receipt.content_generation_digest.clone(),
        coverage_input_digest,
        candidate_owner_paths,
        elapsed_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
        process_count: 1,
    })
}

fn owner_paths_from_rg_output(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    output: &[u8],
    limit: usize,
) -> Result<Vec<String>, String> {
    let mut owners = BTreeSet::new();
    for item in matches_from_rg_output(corpus, output, limit)? {
        owners.insert(item.owner_path);
        if owners.len() == limit {
            break;
        }
    }
    Ok(owners.into_iter().collect())
}

fn matches_from_rg_output(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    output: &[u8],
    limit: usize,
) -> Result<Vec<RuntimeRgMatch>, String> {
    let mut seen = BTreeSet::new();
    let mut matches = Vec::new();
    for line in output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let separator = line
            .iter()
            .position(|byte| *byte == b':')
            .ok_or_else(|| "cold rg emitted a line without a line-number prefix".to_owned())?;
        let line_number = std::str::from_utf8(&line[..separator])
            .map_err(|_| "cold rg line-number prefix is not UTF-8".to_owned())?
            .parse::<u64>()
            .map_err(|_| "cold rg line-number prefix is invalid".to_owned())?;
        let owner = agent_semantic_search::owner_for_corpus_line(&corpus.owner_spans, line_number)
            .ok_or_else(|| "cold rg result is outside the admitted owner corpus".to_owned())?;
        let item = RuntimeRgMatch {
            owner_path: owner.owner_path.clone(),
            owner_line: line_number - owner.start_line + 1,
        };
        if seen.insert((item.owner_path.clone(), item.owner_line)) {
            matches.push(item);
        }
        if matches.len() == limit {
            break;
        }
    }
    Ok(matches)
}

fn coverage_input_digest(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    terms: &BTreeSet<String>,
    limit: u32,
) -> String {
    let mut hasher = blake3::Hasher::new();
    for value in [
        corpus.receipt.content_generation_digest.as_bytes(),
        corpus.receipt.corpus_digest.as_bytes(),
        corpus.receipt.owner_spans_digest.as_bytes(),
    ] {
        hasher.update(&(value.len() as u64).to_le_bytes());
        hasher.update(value);
    }
    for term in terms {
        hasher.update(&(term.len() as u64).to_le_bytes());
        hasher.update(term.as_bytes());
    }
    hasher.update(&limit.to_le_bytes());
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_cold_rg.rs"]
mod tests;
