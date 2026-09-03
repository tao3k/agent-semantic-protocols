//! One bounded ripgrep process over an immutable content-generation corpus.

use std::collections::BTreeSet;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_RG_LINE_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeColdRgReceipt {
    pub content_generation_digest: String,
    pub coverage_input_digest: String,
    pub candidate_owner_paths: Vec<String>,
    pub elapsed_micros: u64,
    pub process_count: u8,
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
        owners.insert(owner.owner_path.clone());
        if owners.len() == limit {
            break;
        }
    }
    Ok(owners.into_iter().collect())
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
