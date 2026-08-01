//! Configuration-independent hook bootstrap and bounded self-repair.
//!
//! This entrypoint deliberately does not parse the hook matcher document.  It
//! can therefore repair an older `asp` binary after the matcher contract has
//! advanced beyond that binary's enum vocabulary.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::time::{Duration, Instant};

use agent_semantic_config::runtime_dev::{RuntimeArtifactMode, parse_runtime_artifact_mode};
const MAX_HOOK_INPUT_BYTES: usize = 1024 * 1024;
const REPAIR_LOCK_WAIT: Duration = Duration::from_secs(90);
const REPAIR_COMMAND_TIMEOUT: Duration = Duration::from_secs(90);
const HOOK_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(15);
const REPAIR_POLL_INTERVAL: Duration = Duration::from_millis(25);
const REPAIR_DEPTH_ENV: &str = "ASP_HOOK_BOOTSTRAP_REPAIR_DEPTH";
const TRACE_ENV: &str = "ASP_HOOK_BOOTSTRAP_TRACE";

/// Run the canonical hook command, repairing the development artifact on a
/// recognized binary/config contract drift before replaying the event once.
#[doc(hidden)]
pub fn run_hook_bootstrap_from_env() -> i32 {
    match run_hook_bootstrap(std::env::args_os().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("[asp-hook] status=failed error={error}");
            2
        }
    }
}

fn run_hook_bootstrap(args: Vec<OsString>) -> Result<i32, String> {
    validate_hook_args(&args)?;
    let input = read_bounded_stdin()?;
    let state_home = agent_semantic_runtime::resolve_state_home()?;
    let hook_args = args
        .iter()
        .map(|arg| {
            arg.to_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "bootstrap hook arguments must be UTF-8".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let hook_input = String::from_utf8(input.clone())
        .map_err(|error| format!("hook payload must be UTF-8 JSON: {error}"))?;
    if let Ok(project_root) = server_project_root(&input) {
        match crate::command::runtime_server_hook_evaluation_client(
            &project_root,
            hook_args.clone(),
            hook_input.clone(),
        ) {
            Ok(output) if !server_hook_output_requires_local_fallback(&output) => {
                if std::env::var_os(TRACE_ENV).is_some() {
                    eprintln!("[asp-hook] route=server-resident-evaluator");
                }
                println!("{output}");
                return Ok(0);
            }
            Ok(_) => {
                if std::env::var_os(TRACE_ENV).is_some() {
                    eprintln!(
                        "[asp-hook] route=local-fallback serverError=activation-unavailable"
                    );
                }
            }
            Err(error) => {
                if std::env::var_os(TRACE_ENV).is_some() {
                    eprintln!(
                        "[asp-hook] route=local-fallback serverError={}",
                        single_line(&error)
                    );
                }
            }
        }
    }
    let error = match crate::command::run_protocol_hook_with_input(hook_args, hook_input) {
        Ok(()) => return Ok(0),
        Err(error) if !is_contract_drift_message(&error) => return Err(error),
        Err(error) => error,
    };
    if std::env::var_os(REPAIR_DEPTH_ENV).is_some() {
        return Err(error);
    }

    match repair_and_replay(&state_home, &args, &input) {
        Ok(replayed) => emit_output(
            replayed,
            Some("status=repaired replay=once authority=dev-root"),
        ),
        Err(repair_error) if event_is_observational(&args) => {
            eprintln!("{error}");
            eprintln!(
                "[asp-hook] status=degraded-open event={} repairError={} policy=observational-liveness",
                hook_event(&args).unwrap_or("unknown"),
                single_line(&repair_error),
            );
            Ok(0)
        }
        Err(repair_error) => {
            eprintln!("{error}");
            eprintln!(
                "[asp-hook] status=repair-failed event={} repairError={} policy=enforcement-fail-closed",
                hook_event(&args).unwrap_or("unknown"),
                single_line(&repair_error),
            );
            Ok(2)
        }
    }
}

fn server_hook_output_requires_local_fallback(output: &str) -> bool {
    output.contains("\"reasonKind\":\"activation-unavailable\"")
        || output.contains("\\\"reasonKind\\\":\\\"activation-unavailable\\\"")
}

fn validate_hook_args(args: &[OsString]) -> Result<(), String> {
    if args.first().and_then(|arg| arg.to_str()) != Some("hook") {
        return Err("bootstrap accepts only `hook <event> ...` dispatch".to_string());
    }
    if hook_event(args).is_none() {
        return Err("bootstrap hook event is missing or non-UTF-8".to_string());
    }
    Ok(())
}

fn read_bounded_stdin() -> Result<Vec<u8>, String> {
    let mut input = Vec::new();
    std::io::stdin()
        .take((MAX_HOOK_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .map_err(|error| format!("read hook stdin: {error}"))?;
    if input.len() > MAX_HOOK_INPUT_BYTES {
        return Err(format!(
            "hook payload exceeds {MAX_HOOK_INPUT_BYTES} byte bootstrap bound"
        ));
    }
    Ok(input)
}

fn server_project_root(input: &[u8]) -> Result<PathBuf, String> {
    let payload: serde_json::Value = serde_json::from_slice(input)
        .map_err(|error| format!("hook payload must be JSON before server routing: {error}"))?;
    for field in ["cwd", "project_root", "projectRoot", "workspace"] {
        if let Some(path) = payload.get(field).and_then(serde_json::Value::as_str)
            && !path.trim().is_empty()
        {
            return Ok(PathBuf::from(path));
        }
    }
    std::env::current_dir().map_err(|error| format!("failed to resolve hook workspace: {error}"))
}

fn run_hook_attempt(executable: &Path, args: &[OsString], input: &[u8]) -> Result<Output, String> {
    agent_semantic_runtime::runtime_block_on_current_thread(
        agent_semantic_runtime::hook_process_runtime::run_hook_process(
            agent_semantic_runtime::hook_process_runtime::HookProcessRequest {
                executable,
                args,
                stdin: input,
                timeout: HOOK_ATTEMPT_TIMEOUT,
                current_dir: None,
                environment: Some((REPAIR_DEPTH_ENV, "1")),
                discard_stdout: false,
            },
        ),
    )?
}

fn is_contract_drift(output: &Output) -> bool {
    if output.status.success() {
        return false;
    }
    is_contract_drift_message(&String::from_utf8_lossy(&output.stderr))
}

fn is_contract_drift_message(message: &str) -> bool {
    message.contains("hook matcher config freshness gate failed")
        || message.contains("hook resident config freshness gate failed")
        || message.contains("hook language provider projection freshness gate failed")
}

fn repair_and_replay(state_home: &Path, args: &[OsString], input: &[u8]) -> Result<Output, String> {
    if std::env::var_os(REPAIR_DEPTH_ENV).is_some() {
        return Err("repair recursion was rejected".to_string());
    }
    let _guard = agent_semantic_runtime::runtime_block_on_current_thread(
        BootstrapRepairGuard::acquire(state_home, REPAIR_LOCK_WAIT),
    )??;
    let bootstrap = state_home.join("runtime/bin/asp");

    // A concurrent hook may have completed publication while this process was
    // waiting for the bootstrap lock.  Recheck before starting a build.
    let concurrent = run_hook_attempt(&bootstrap, args, input)?;
    if concurrent.status.success() {
        return Ok(concurrent);
    }
    if !is_contract_drift(&concurrent) {
        return Err(format!(
            "hook failure changed while awaiting repair lock: status={}",
            concurrent.status
        ));
    }

    run_authorized_repair(state_home)?;
    let replayed = run_hook_attempt(&bootstrap, args, input)?;
    if replayed.status.success() {
        Ok(replayed)
    } else {
        Err(format!(
            "one replay after repair failed: status={} stderr={}",
            replayed.status,
            single_line(&String::from_utf8_lossy(&replayed.stderr))
        ))
    }
}

fn run_authorized_repair(state_home: &Path) -> Result<(), String> {
    let config_path = state_home.join("asp.toml");
    let input = std::fs::read_to_string(&config_path).map_err(|error| {
        format!(
            "read bootstrap authority {}: {error}",
            config_path.display()
        )
    })?;
    let RuntimeArtifactMode::Dev { root } = parse_runtime_artifact_mode(&input)? else {
        return Err(
            "release mode has no authorized development checkout for hook artifact repair"
                .to_string(),
        );
    };
    let root = root.canonicalize().map_err(|error| {
        format!(
            "canonicalize configured hook repair dev root {}: {error}",
            root.display()
        )
    })?;
    let justfile = root.join("Justfile");
    if !justfile.is_file() {
        return Err(format!(
            "configured hook repair dev root has no Justfile: {}",
            justfile.display()
        ));
    }
    let runtime_bin = state_home.join("runtime/bin");
    let executable =
        agent_semantic_runtime::hook_process_runtime::hook_development_installer_executable();
    let args = agent_semantic_runtime::hook_process_runtime::hook_development_installer_args(
        &root,
        &justfile,
        &runtime_bin,
    );
    let output = agent_semantic_runtime::runtime_block_on_current_thread(
        agent_semantic_runtime::hook_process_runtime::run_hook_process(
            agent_semantic_runtime::hook_process_runtime::HookProcessRequest {
                executable: &executable,
                args: &args,
                stdin: &[],
                timeout: REPAIR_COMMAND_TIMEOUT,
                current_dir: Some(&root),
                environment: Some((REPAIR_DEPTH_ENV, "1")),
                discard_stdout: true,
            },
        ),
    )??;
    if !output.status.success() {
        return Err(format!(
            "authorized hook repair failed: status={} stderr={}",
            output.status,
            single_line(&String::from_utf8_lossy(&output.stderr))
        ));
    }
    Ok(())
}

fn emit_output(output: Output, receipt: Option<&str>) -> Result<i32, String> {
    std::io::stdout()
        .write_all(&output.stdout)
        .map_err(|error| format!("write hook stdout: {error}"))?;
    std::io::stderr()
        .write_all(&output.stderr)
        .map_err(|error| format!("write hook stderr: {error}"))?;
    if let Some(receipt) = receipt {
        eprintln!("[asp-hook] {receipt}");
    }
    Ok(exit_code(output.status))
}

fn hook_event(args: &[OsString]) -> Option<&str> {
    args.get(1).and_then(|arg| arg.to_str())
}

fn event_is_observational(args: &[OsString]) -> bool {
    matches!(
        hook_event(args),
        Some(
            "session-start"
                | "user-prompt"
                | "post-tool"
                | "subagent-start"
                | "subagent-stop"
                | "stop"
                | "notification"
        )
    )
}

fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(2)
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct BootstrapRepairGuard {
    file: std::fs::File,
}

impl BootstrapRepairGuard {
    async fn acquire(state_home: &Path, timeout: Duration) -> Result<Self, String> {
        let lock_dir = state_home.join("runtime/locks");
        std::fs::create_dir_all(&lock_dir)
            .map_err(|error| format!("create bootstrap repair lock directory: {error}"))?;
        let lock_path = lock_dir.join("hook-bootstrap-repair.v1.lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| {
                format!(
                    "open bootstrap repair lock {}: {error}",
                    lock_path.display()
                )
            })?;
        let started = Instant::now();
        loop {
            #[cfg(unix)]
            {
                use std::os::fd::AsRawFd;
                let status =
                    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                if status == 0 {
                    return Ok(Self { file });
                }
            }
            #[cfg(not(unix))]
            return Ok(Self { file });
            if started.elapsed() >= timeout {
                return Err(format!(
                    "bootstrap repair lock wait exceeded {} seconds: {}",
                    timeout.as_secs(),
                    lock_path.display()
                ));
            }
            tokio::time::sleep(REPAIR_POLL_INTERVAL).await;
        }
    }
}

impl Drop for BootstrapRepairGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/hook_bootstrap.rs"]
mod tests;
