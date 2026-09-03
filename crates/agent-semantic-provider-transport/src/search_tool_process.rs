//! Bounded Runtime-owned execution for `fd` inventory and cold `rg` queries.

#[cfg(test)]
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::process::Stdio;
#[cfg(test)]
use tokio::io::{AsyncRead, AsyncReadExt};

const MAX_INVENTORY_BYTES: usize = 16 * 1024 * 1024;
const MAX_RG_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
#[cfg(test)]
const MAX_STDERR_BYTES: usize = 256 * 1024;
const MAX_COLD_RG_CORPUS_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct FdInventoryOutput {
    pub owner_paths: Vec<String>,
    pub receipt: FdInventoryReceipt,
}

#[derive(Clone, Debug)]
pub struct FdInventoryReceipt {
    pub backend: &'static str,
    pub elapsed: Duration,
    pub owner_count: usize,
}

#[derive(Debug)]
pub struct RgColdQueryOutput {
    pub output: bytes::Bytes,
    pub receipt: RgColdQueryReceipt,
}

#[derive(Clone, Debug)]
pub struct RgColdQueryReceipt {
    pub backend: &'static str,
    pub corpus_digest: String,
    pub elapsed: Duration,
    pub input_bytes: usize,
    pub matched_lines: usize,
    pub output_bytes: usize,
    pub output_sha256: String,
}

#[derive(Clone, Debug)]
pub struct ValidatedColdRgCorpus {
    path: PathBuf,
    digest: String,
    bytes: Bytes,
}

impl ValidatedColdRgCorpus {
    pub fn open(path: &Path, expected_digest: &str) -> Result<Self, String> {
        let bytes = validate_corpus(path, expected_digest)?;
        Ok(Self {
            path: path.to_path_buf(),
            digest: expected_digest.to_owned(),
            bytes,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

pub async fn run_fd_inventory(
    workspace_root: &Path,
    timeout: Duration,
) -> Result<FdInventoryOutput, String> {
    let started = Instant::now();
    let workspace_root = workspace_root.to_path_buf();
    let owner_paths =
        tokio::task::spawn_blocking(move || build_resident_fd_inventory(&workspace_root, timeout))
            .await
            .map_err(|error| format!("join resident fd inventory: {error}"))??;
    let owner_count = owner_paths.len();
    Ok(FdInventoryOutput {
        owner_paths,
        receipt: FdInventoryReceipt {
            backend: "resident-fd-ignore-walk",
            elapsed: started.elapsed(),
            owner_count,
        },
    })
}

fn build_resident_fd_inventory(
    workspace_root: &Path,
    timeout: Duration,
) -> Result<Vec<String>, String> {
    let started = Instant::now();
    let mut paths = Vec::new();
    let mut retained_bytes = 0_usize;
    let walker = ignore::WalkBuilder::new(workspace_root)
        .hidden(false)
        .follow_links(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();
    for entry in walker {
        if started.elapsed() >= timeout {
            return Err(format!(
                "resident fd inventory timed out after {}ms",
                timeout.as_millis()
            ));
        }
        let entry = entry.map_err(|error| format!("walk resident fd inventory: {error}"))?;
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let relative = entry.path().strip_prefix(workspace_root).map_err(|error| {
            format!(
                "resident fd inventory path escaped workspace: path={} error={error}",
                entry.path().display()
            )
        })?;
        let relative = relative
            .to_str()
            .ok_or_else(|| "resident fd inventory path is not UTF-8".to_owned())?;
        validate_relative_path(relative)?;
        retained_bytes = retained_bytes.saturating_add(relative.len().saturating_add(1));
        paths.push(relative.to_owned());
        if retained_bytes > MAX_INVENTORY_BYTES {
            return Err("resident fd inventory exceeds its byte envelope".to_owned());
        }
    }
    paths.sort_unstable();
    if paths.is_empty() {
        return Err("resident fd inventory is empty".to_owned());
    }
    if let Some(duplicate) = paths.windows(2).find(|window| window[0] == window[1]) {
        return Err(format!(
            "resident fd inventory repeated owner path: {}",
            duplicate[0]
        ));
    }
    Ok(paths)
}

pub async fn run_rg_cold_query(
    corpus: &ValidatedColdRgCorpus,
    query: &str,
    timeout: Duration,
) -> Result<RgColdQueryOutput, String> {
    if query.is_empty() {
        return Err("cold rg query is empty".to_owned());
    }
    let started = Instant::now();
    let corpus_digest = corpus.digest().to_owned();
    let query = query.as_bytes().to_vec();
    // The immutable corpus is bounded and the scan checks its own deadline.
    // Crossing into Tokio's blocking pool costs more than the SIMD byte scan
    // for ordinary source generations and introduces scheduler-tail latency.
    let scanned = scan_cold_rg_corpus(corpus.bytes(), &query, timeout, MAX_RG_OUTPUT_BYTES)?;
    let elapsed = started.elapsed();
    Ok(RgColdQueryOutput {
        output: scanned.output,
        receipt: RgColdQueryReceipt {
            backend: "resident-ripgrep-fixed-string",
            corpus_digest,
            elapsed,
            input_bytes: corpus.bytes().len(),
            matched_lines: scanned.matched_lines,
            output_bytes: scanned.output_bytes,
            output_sha256: scanned.output_sha256,
        },
    })
}

struct ColdRgScan {
    output: Bytes,
    matched_lines: usize,
    output_bytes: usize,
    output_sha256: String,
}

fn scan_cold_rg_corpus(
    corpus: &[u8],
    query: &[u8],
    timeout: Duration,
    output_limit: usize,
) -> Result<ColdRgScan, String> {
    let started = Instant::now();
    let finder = memchr::memmem::Finder::new(query);
    let mut output = BytesMut::with_capacity(output_limit.min(64 * 1024));
    let mut output_hasher = Sha256::new();
    let mut output_bytes = 0_usize;
    let mut matched_lines = 0_usize;
    for (line_index, line) in crate::byte_text::split_lf_lines(corpus).enumerate() {
        if started.elapsed() >= timeout {
            return Err(format!(
                "resident cold rg timed out after {}ms",
                timeout.as_millis()
            ));
        }
        let Some(column) = finder.find(line) else {
            continue;
        };
        matched_lines = matched_lines.saturating_add(1);
        let prefix = format!(
            "{}:{}:",
            line_index.saturating_add(1),
            column.saturating_add(1)
        );
        let record_len = prefix.len().saturating_add(line.len()).saturating_add(1);
        output_bytes = output_bytes.saturating_add(record_len);
        if output_bytes > output_limit {
            return Err(format!(
                "resident cold rg output exceeds {} bytes",
                output_limit
            ));
        }
        output.extend_from_slice(prefix.as_bytes());
        output.extend_from_slice(line);
        output.extend_from_slice(b"\n");
    }
    output_hasher.update(&output);
    Ok(ColdRgScan {
        output: output.freeze(),
        matched_lines,
        output_bytes,
        output_sha256: format!("{:x}", output_hasher.finalize()),
    })
}

#[cfg(test)]
async fn run_reference_search_tool(
    program: &Path,
    workspace_root: &Path,
    args: Vec<String>,
    timeout: Duration,
    max_stdout_bytes: usize,
) -> Result<SearchToolOutput, String> {
    let program = resolve_program(program)?;
    let mut command = tokio::process::Command::new(&program);
    command
        .args(args)
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("spawn search tool {}: {error}", program.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "search tool stdout pipe is unavailable".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "search tool stderr pipe is unavailable".to_owned())?;
    let stdout_task = tokio::spawn(read_bounded_async(stdout, max_stdout_bytes));
    let stderr_task = tokio::spawn(read_bounded_async(stderr, MAX_STDERR_BYTES));
    let (status, timed_out) = wait_for_search_tool(&mut child, timeout).await?;
    let stdout = join_bounded_read(stdout_task, "stdout").await?;
    let stderr = join_bounded_read(stderr_task, "stderr").await?;
    if timed_out {
        return Err(format!(
            "search tool {} timed out after {}ms (stdoutBytes={} stderrBytes={})",
            program.display(),
            timeout.as_millis(),
            stdout.total_bytes,
            stderr.total_bytes,
        ));
    }
    if !status.success() || stdout.truncated || stderr.truncated {
        return Err(format!(
            "reference search tool failed: program={} status={:?} stdoutTruncated={} stderrTruncated={} stderr={}",
            program.display(),
            status.code(),
            stdout.truncated,
            stderr.truncated,
            String::from_utf8_lossy(&stderr.bytes)
        ));
    }
    Ok(SearchToolOutput {
        stdout: stdout.bytes,
    })
}

#[cfg(test)]
async fn wait_for_search_tool(
    child: &mut tokio::process::Child,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, bool), String> {
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(status) => Ok((
            status.map_err(|error| format!("wait for search tool: {error}"))?,
            false,
        )),
        Err(_) => {
            child
                .start_kill()
                .map_err(|error| format!("kill timed-out search tool: {error}"))?;
            let status = child
                .wait()
                .await
                .map_err(|error| format!("reap timed-out search tool: {error}"))?;
            Ok((status, true))
        }
    }
}

#[derive(Debug)]
#[cfg(test)]
struct SearchToolOutput {
    stdout: Bytes,
}

#[derive(Debug)]
#[cfg(test)]
struct BoundedRead {
    bytes: Bytes,
    total_bytes: usize,
    truncated: bool,
}

#[cfg(test)]
async fn read_bounded_async(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<BoundedRead, std::io::Error> {
    let mut bytes = BytesMut::with_capacity(limit.min(64 * 1024));
    let mut total_bytes = 0_usize;
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(count);
        if bytes.len() < limit {
            let retained = (limit - bytes.len()).min(count);
            bytes.extend_from_slice(&chunk[..retained]);
        }
    }
    Ok(BoundedRead {
        bytes: bytes.freeze(),
        total_bytes,
        truncated: total_bytes > limit,
    })
}

#[cfg(test)]
async fn join_bounded_read(
    task: tokio::task::JoinHandle<Result<BoundedRead, std::io::Error>>,
    stream: &str,
) -> Result<BoundedRead, String> {
    task.await
        .map_err(|error| format!("join search-tool {stream} reader: {error}"))?
        .map_err(|error| format!("read search-tool {stream}: {error}"))
}

#[cfg(test)]
fn resolve_program(program: &Path) -> Result<PathBuf, String> {
    if program.is_absolute() || program.components().count() > 1 {
        return program
            .canonicalize()
            .map_err(|error| format!("resolve search tool {}: {error}", program.display()));
    }
    let path = std::env::var_os("PATH").ok_or_else(|| "PATH is unavailable".to_owned())?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
        .ok_or_else(|| format!("search tool is unavailable on PATH: {}", program.display()))
}

#[cfg(test)]
fn parse_fd_inventory(output: &[u8]) -> Result<Vec<String>, String> {
    let mut paths = BTreeSet::new();
    for record in output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let path = std::str::from_utf8(record)
            .map_err(|error| format!("fd inventory path is not UTF-8: {error}"))?;
        let path = path.strip_prefix("./").unwrap_or(path);
        validate_relative_path(path)?;
        if !paths.insert(path.to_owned()) {
            return Err(format!("fd inventory repeated owner path: {path}"));
        }
    }
    if paths.is_empty() {
        return Err("fd inventory is empty".to_owned());
    }
    Ok(paths.into_iter().collect())
}

fn validate_corpus(path: &Path, expected_digest: &str) -> Result<Bytes, String> {
    if !path.is_absolute() || !expected_digest.starts_with("blake3-256:") {
        return Err("cold rg corpus authority is incomplete".to_owned());
    }
    let bytes = std::fs::read(path).map_err(|error| format!("read cold rg corpus: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_COLD_RG_CORPUS_BYTES {
        return Err("cold rg corpus is outside the bounded envelope".to_owned());
    }
    let actual = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(&bytes).value
    );
    if actual != expected_digest {
        return Err("cold rg corpus digest mismatch".to_owned());
    }
    Ok(Bytes::from(bytes))
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "search tool owner path is not normalized and relative: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/search_tool_process.rs"]
mod tests;
