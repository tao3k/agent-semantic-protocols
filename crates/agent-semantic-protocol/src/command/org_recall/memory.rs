use super::model::{OrgPlanCandidate, RankedOrgPlan};
use agent_semantic_runtime::project_state_paths;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[derive(Serialize)]
struct MemoryRankRequest {
    plans: Vec<MemoryRankPlan>,
}

#[derive(Serialize)]
struct MemoryWorkerRankRequest<'a> {
    command: &'static str,
    payload: &'a MemoryRankRequest,
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
    #[serde(rename = "taskCandidates")]
    task_candidates: Vec<MemoryRankTaskCandidate>,
}

#[derive(Serialize)]
struct MemoryRankTaskCandidate {
    kind: String,
    status: String,
    title: String,
    section: Option<String>,
    #[serde(rename = "sourceLine")]
    source_line: Option<usize>,
}

#[derive(Deserialize)]
struct MemoryRankResponse {
    plans: Vec<MemoryRankRow>,
}

#[derive(Deserialize)]
struct MemoryRankRow {
    id: String,
    score: f64,
    #[serde(rename = "contextScore")]
    context_score: f64,
    #[serde(rename = "memoryScore")]
    memory_score: f64,
    #[serde(rename = "recencyScore")]
    recency_score: f64,
}

pub(super) struct MemoryRankOptions<'a> {
    pub(super) project: &'a str,
    pub(super) session: Option<&'a str>,
    pub(super) branch: Option<&'a str>,
    pub(super) state: Option<&'a Path>,
    pub(super) embedding_dim: Option<&'a str>,
    pub(super) top_k: usize,
    pub(super) project_root: &'a Path,
}

pub(super) struct MemoryRankedPlans {
    pub(super) plans: Vec<RankedOrgPlan>,
    pub(super) transport: &'static str,
}

pub(super) fn rank_plans_with_memory_engine(
    candidates: &[OrgPlanCandidate],
    options: MemoryRankOptions<'_>,
) -> Result<MemoryRankedPlans, String> {
    let MemoryRankOptions {
        project,
        session,
        branch,
        state,
        embedding_dim,
        top_k,
        project_root,
    } = options;
    if candidates.is_empty() {
        return Ok(MemoryRankedPlans {
            plans: Vec::new(),
            transport: "none",
        });
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
                task_candidates: candidate
                    .task_candidates
                    .iter()
                    .map(|task| MemoryRankTaskCandidate {
                        kind: task.kind.clone(),
                        status: task.status.clone(),
                        title: task.title.clone(),
                        section: task.section.clone(),
                        source_line: task.source_line,
                    })
                    .collect(),
            })
            .collect(),
    };
    let request_body = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to encode memory score request: {error}"))?;
    let mut rank_args = vec![
        "rank-plans".to_string(),
        "--project".to_string(),
        project.to_string(),
        "--top-k".to_string(),
        top_k.to_string(),
    ];
    push_optional_string_arg(&mut rank_args, "--session", session);
    push_optional_string_arg(&mut rank_args, "--branch", branch);
    push_optional_path_arg(&mut rank_args, "--state", state, project_root);
    push_optional_string_arg(&mut rank_args, "--embedding-dim", embedding_dim);
    let worker_request = MemoryWorkerRankRequest {
        command: "rank-plans",
        payload: &request,
        project,
        session,
        branch,
        top_k,
        state: state.map(|value| absolute_path(value, project_root).display().to_string()),
        embedding_dim,
    };
    let (response, transport) = if let Ok(socket_path) = env::var("ASP_MEMORY_ENGINE_SOCKET") {
        let output = run_asp_memory_engine_worker(&socket_path, &worker_request)?;
        (decode_memory_rank_response(&output)?, "socket:env")
    } else if memory_engine_auto_socket_enabled() {
        match run_asp_memory_engine_auto_worker_checked(
            &worker_request,
            project_root,
            validate_memory_rank_response,
        ) {
            Ok(output) => (decode_memory_rank_response(&output)?, "socket:auto"),
            Err(_) => {
                let output = run_asp_memory_engine(&rank_args, &request_body, project_root)?;
                (
                    decode_memory_rank_response(&output)?,
                    "process:auto-fallback",
                )
            }
        }
    } else {
        let output = run_asp_memory_engine(&rank_args, &request_body, project_root)?;
        (decode_memory_rank_response(&output)?, "process")
    };
    let candidates_by_id: BTreeMap<_, _> = candidates
        .iter()
        .map(|candidate| (candidate.plan_id(), candidate.clone()))
        .collect();
    let plans = response
        .plans
        .into_iter()
        .filter_map(|row| {
            candidates_by_id
                .get(&row.id)
                .cloned()
                .map(|candidate| RankedOrgPlan {
                    candidate,
                    score: row.score,
                    context_score: row.context_score,
                    memory_score: row.memory_score,
                    recency_score: row.recency_score,
                })
        })
        .collect();
    Ok(MemoryRankedPlans { plans, transport })
}

fn decode_memory_rank_response(output: &[u8]) -> Result<MemoryRankResponse, String> {
    serde_json::from_slice(output)
        .map_err(|error| format!("failed to decode asp-memory-engine rank-plans output: {error}"))
}

fn validate_memory_rank_response(output: &[u8]) -> Result<(), String> {
    decode_memory_rank_response(output).map(|_| ())
}

pub(super) fn run_asp_memory_engine_auto_worker_checked<T, F>(
    request: &T,
    project_root: &Path,
    validate: F,
) -> Result<Vec<u8>, String>
where
    T: Serialize,
    F: Fn(&[u8]) -> Result<(), String>,
{
    let socket_path = default_memory_engine_socket(project_root);
    match run_asp_memory_engine_worker_path(&socket_path, request) {
        Ok(output) => match validate(&output) {
            Ok(()) => return Ok(output),
            Err(_) => remove_stale_socket(&socket_path)?,
        },
        Err(_) => remove_stale_socket(&socket_path)?,
    }
    start_memory_engine_worker(&socket_path, project_root)?;
    let output = run_asp_memory_engine_worker_path(&socket_path, request)?;
    if let Err(error) = validate(&output) {
        let _ = remove_stale_socket(&socket_path);
        return Err(error);
    }
    Ok(output)
}

pub(super) fn run_asp_memory_engine_worker<T: Serialize>(
    socket_path: &str,
    request: &T,
) -> Result<Vec<u8>, String> {
    run_asp_memory_engine_worker_path(Path::new(socket_path), request)
}

fn run_asp_memory_engine_worker_path<T: Serialize>(
    socket_path: &Path,
    request: &T,
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

fn start_memory_engine_worker(socket_path: &Path, project_root: &Path) -> Result<(), String> {
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create memory worker socket dir: {error}"))?;
    }
    let mut child = asp_memory_engine_command(project_root)?
        .args(["worker", "--socket"])
        .arg(socket_path)
        .current_dir(project_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to start asp-memory-engine worker: {error}"))?;
    wait_for_memory_engine_socket(socket_path).inspect_err(|_| {
        let _ = child.kill();
    })
}

fn wait_for_memory_engine_socket(socket_path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        let mut last_error = None;
        for _ in 0..40 {
            match UnixStream::connect(socket_path) {
                Ok(_) => return Ok(()),
                Err(error) => {
                    last_error = Some(error);
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }
        Err(format!(
            "asp-memory-engine worker did not create socket {}: {}",
            socket_path.display(),
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "unknown error".to_string())
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = socket_path;
        Err(
            "default asp-memory-engine socket worker is only supported on Unix platforms"
                .to_string(),
        )
    }
}

pub(super) fn run_asp_memory_engine(
    args: &[String],
    stdin: &[u8],
    project_root: &Path,
) -> Result<Vec<u8>, String> {
    let mut child = asp_memory_engine_command(project_root)?
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

fn asp_memory_engine_command(project_root: &Path) -> Result<Command, String> {
    if let Ok(binary) = env::var("ASP_MEMORY_ENGINE") {
        return Ok(Command::new(binary));
    }
    if let Some(command) = source_memory_engine_command(project_root) {
        return Ok(command);
    }
    if let Some(binary) = project_memory_engine_binary(project_root) {
        return Ok(Command::new(binary));
    }
    if command_exists("asp-memory-engine") {
        return Ok(Command::new("asp-memory-engine"));
    }
    if let Some(command) = source_memory_engine_command(Path::new(env!("CARGO_MANIFEST_DIR"))) {
        return Ok(command);
    }
    Err(
        "asp org recall plans requires ASP_MEMORY_ENGINE, a local packages/python workspace, a project packaged asp-memory-engine, or `asp-memory-engine` on PATH"
            .to_string(),
    )
}

fn source_memory_engine_command(project_root: &Path) -> Option<Command> {
    let root_packages_python = absolute_path(project_root, project_root).join("packages/python");
    let source_packages_python =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/python");
    let packages_python = if root_packages_python.join("pyproject.toml").is_file() {
        root_packages_python
    } else {
        source_packages_python
    };
    if packages_python.join("pyproject.toml").is_file() {
        let mut command = Command::new("uv");
        command
            .args(["run", "--project"])
            .arg(packages_python)
            .arg("--frozen")
            .arg("asp-memory-engine");
        return Some(command);
    }
    None
}

fn project_memory_engine_binary(project_root: &Path) -> Option<PathBuf> {
    let project_root = absolute_path(project_root, project_root);
    let mut candidates = Vec::new();
    if let Ok(paths) = project_state_paths(&project_root) {
        let artifacts_bin = paths.artifacts_dir.join("bin");
        candidates.push(artifacts_bin.join("asp-memory-engine-current"));
        candidates.push(artifacts_bin.join("asp-memory-engine"));
    }
    candidates.push(project_root.join(".bin/asp-memory-engine"));
    candidates.into_iter().find(|candidate| candidate.is_file())
}

pub(super) fn memory_engine_auto_socket_enabled() -> bool {
    !matches!(
        env::var("ASP_MEMORY_ENGINE_AUTO_SOCKET")
            .unwrap_or_else(|_| "1".to_string())
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off" | "no"
    )
}

fn default_memory_engine_socket(project_root: &Path) -> PathBuf {
    let socket_dir = env::var_os("ASP_MEMORY_ENGINE_SOCKET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    socket_dir.join(format!(
        "asp-memory-engine-{:016x}.sock",
        stable_project_hash(&absolute_path(project_root, project_root))
    ))
}

fn stable_project_hash(path: &Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in path.display().to_string().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn remove_stale_socket(socket_path: &Path) -> Result<(), String> {
    match fs::remove_file(socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale memory worker socket {}: {error}",
            socket_path.display()
        )),
    }
}

pub(super) fn push_optional_string_arg(args: &mut Vec<String>, flag: &str, value: Option<&str>) {
    if let Some(value) = value {
        args.extend([flag.to_string(), value.to_string()]);
    }
}

pub(super) fn push_optional_path_arg(
    args: &mut Vec<String>,
    flag: &str,
    value: Option<&Path>,
    root: &Path,
) {
    if let Some(value) = value {
        args.extend([
            flag.to_string(),
            absolute_path(value, root).display().to_string(),
        ]);
    }
}

pub(super) fn absolute_path(value: &Path, root: &Path) -> PathBuf {
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
