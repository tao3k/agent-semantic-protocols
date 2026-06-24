use super::model::{OrgPlanCandidate, RankedOrgPlan};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Serialize)]
struct MemoryRankRequest {
    plans: Vec<MemoryRankPlan>,
}

#[derive(Serialize)]
struct MemoryWorkerRankRequest<'a> {
    command: &'static str,
    payload: &'a MemoryRankRequest,
    intent: &'a str,
    project: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<&'a str>,
    #[serde(rename = "topK")]
    top_k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(rename = "embeddingDim", skip_serializing_if = "Option::is_none")]
    embedding_dim: Option<&'a str>,
}

#[derive(Serialize)]
struct MemoryRankPlan {
    id: String,
    path: String,
    title: String,
    todo: String,
    mtime: f64,
    properties: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct MemoryRankResponse {
    plans: Vec<MemoryRankRow>,
}

#[derive(Deserialize)]
struct MemoryRankRow {
    id: String,
    score: f64,
    #[serde(rename = "textScore")]
    text_score: f64,
    #[serde(rename = "memoryScore")]
    memory_score: f64,
    #[serde(rename = "recencyScore")]
    recency_score: f64,
    #[serde(rename = "intentScore")]
    intent_score: f64,
}

pub(super) struct MemoryRankOptions<'a> {
    pub(super) intent: &'a str,
    pub(super) project: &'a str,
    pub(super) session: Option<&'a str>,
    pub(super) branch: Option<&'a str>,
    pub(super) state: Option<&'a Path>,
    pub(super) embedding_dim: Option<&'a str>,
    pub(super) top_k: usize,
    pub(super) project_root: &'a Path,
}

pub(super) fn rank_plans_with_memory_engine(
    candidates: &[OrgPlanCandidate],
    options: MemoryRankOptions<'_>,
) -> Result<Vec<RankedOrgPlan>, String> {
    let MemoryRankOptions {
        intent,
        project,
        session,
        branch,
        state,
        embedding_dim,
        top_k,
        project_root,
    } = options;
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let request = MemoryRankRequest {
        plans: candidates
            .iter()
            .map(|candidate| MemoryRankPlan {
                id: candidate.plan_id(),
                path: candidate.path.display().to_string(),
                title: candidate.title.clone(),
                todo: candidate.todo.clone(),
                mtime: candidate.mtime,
                properties: candidate.properties.clone(),
            })
            .collect(),
    };
    let request_body = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to encode memory score request: {error}"))?;
    let mut rank_args = vec![
        "rank-plans".to_string(),
        "--intent".to_string(),
        intent.to_string(),
        "--project".to_string(),
        project.to_string(),
        "--top-k".to_string(),
        top_k.to_string(),
    ];
    push_optional_string_arg(&mut rank_args, "--session", session);
    push_optional_string_arg(&mut rank_args, "--branch", branch);
    push_optional_path_arg(&mut rank_args, "--state", state, project_root);
    push_optional_string_arg(&mut rank_args, "--embedding-dim", embedding_dim);
    let output = if let Ok(socket_path) = env::var("ASP_MEMORY_ENGINE_SOCKET") {
        let worker_request = MemoryWorkerRankRequest {
            command: "rank-plans",
            payload: &request,
            intent,
            project,
            session,
            branch,
            top_k,
            state: state.map(|value| absolute_path(value, project_root).display().to_string()),
            embedding_dim,
        };
        run_asp_memory_engine_worker(&socket_path, &worker_request)?
    } else {
        run_asp_memory_engine(&rank_args, &request_body, project_root)?
    };
    let response: MemoryRankResponse = serde_json::from_slice(&output).map_err(|error| {
        format!("failed to decode asp-memory-engine rank-plans output: {error}")
    })?;
    let candidates_by_id: BTreeMap<_, _> = candidates
        .iter()
        .map(|candidate| (candidate.plan_id(), candidate.clone()))
        .collect();
    Ok(response
        .plans
        .into_iter()
        .filter_map(|row| {
            candidates_by_id
                .get(&row.id)
                .cloned()
                .map(|candidate| RankedOrgPlan {
                    candidate,
                    score: row.score,
                    text_score: row.text_score,
                    memory_score: row.memory_score,
                    recency_score: row.recency_score,
                    intent_score: row.intent_score,
                })
        })
        .collect())
}

fn run_asp_memory_engine_worker(
    socket_path: &str,
    request: &MemoryWorkerRankRequest<'_>,
) -> Result<Vec<u8>, String> {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        let mut stream = UnixStream::connect(socket_path)
            .map_err(|error| format!("failed to connect ASP_MEMORY_ENGINE_SOCKET: {error}"))?;
        serde_json::to_writer(&mut stream, request)
            .map_err(|error| format!("failed to encode memory worker request: {error}"))?;
        stream
            .write_all(b"\n")
            .map_err(|error| format!("failed to write memory worker request: {error}"))?;
        stream
            .flush()
            .map_err(|error| format!("failed to flush memory worker request: {error}"))?;
        let mut reader = BufReader::new(stream);
        let mut line = Vec::new();
        reader
            .read_until(b'\n', &mut line)
            .map_err(|error| format!("failed to read memory worker response: {error}"))?;
        if line.is_empty() {
            return Err("memory worker returned no response".to_string());
        }
        Ok(line)
    }
    #[cfg(not(unix))]
    {
        let _ = socket_path;
        let _ = request;
        Err("ASP_MEMORY_ENGINE_SOCKET is only supported on Unix platforms".to_string())
    }
}

fn run_asp_memory_engine(
    args: &[String],
    stdin: &[u8],
    project_root: &Path,
) -> Result<Vec<u8>, String> {
    let mut child = asp_memory_engine_command()?
        .args(args)
        .current_dir(project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "failed to run asp-memory-engine {}: {error}",
                args.join(" ")
            )
        })?;
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open asp-memory-engine stdin".to_string())?;
    child_stdin
        .write_all(stdin)
        .map_err(|error| format!("failed to write asp-memory-engine stdin: {error}"))?;
    drop(child_stdin);
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for asp-memory-engine: {error}"))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(format!(
        "asp-memory-engine {} failed with status {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn asp_memory_engine_command() -> Result<Command, String> {
    if let Ok(binary) = env::var("ASP_MEMORY_ENGINE") {
        return Ok(Command::new(binary));
    }
    if command_exists("asp-memory-engine") {
        return Ok(Command::new("asp-memory-engine"));
    }
    let packages_python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/python");
    if packages_python.join("pyproject.toml").is_file() {
        let mut command = Command::new("uv");
        command
            .args(["run", "--project"])
            .arg(packages_python)
            .arg("--frozen")
            .arg("asp-memory-engine");
        return Ok(command);
    }
    Err(
        "asp org recall plans requires `asp-memory-engine` or a local packages/python workspace"
            .to_string(),
    )
}

fn push_optional_string_arg(args: &mut Vec<String>, flag: &str, value: Option<&str>) {
    if let Some(value) = value {
        args.extend([flag.to_string(), value.to_string()]);
    }
}

fn push_optional_path_arg(args: &mut Vec<String>, flag: &str, value: Option<&Path>, root: &Path) {
    if let Some(value) = value {
        args.extend([
            flag.to_string(),
            absolute_path(value, root).display().to_string(),
        ]);
    }
}

fn absolute_path(value: &Path, root: &Path) -> PathBuf {
    if value.is_absolute() {
        value.to_path_buf()
    } else {
        root.join(value)
    }
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {command} >/dev/null 2>&1"))
        .status()
        .is_ok_and(|status| status.success())
}
