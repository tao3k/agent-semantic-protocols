use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(1);
const CODEX_DECISION_CONTEXT_PREFIX: &str = "[agent-hook-decision] ";

#[derive(Clone, Copy, Debug)]
pub struct HookBinaryCandidateValidation<'a> {
    pub hook_binary_path: &'a Path,
    pub generation_path: &'a Path,
    pub generation_digest: &'a str,
    pub state_home: &'a Path,
}

fn valid_reader_validation_receipt(context: &str, generation_digest: &str) -> bool {
    let Some(encoded) = context.strip_prefix(CODEX_DECISION_CONTEXT_PREFIX) else {
        return false;
    };
    let Ok(receipt) = serde_json::from_str::<serde_json::Value>(encoded) else {
        return false;
    };
    let string = |field| receipt.get(field).and_then(serde_json::Value::as_str);
    let boolean = |field| receipt.get(field).and_then(serde_json::Value::as_bool);
    let trusted_reader_evidence = matches!(
        (
            string("backend"),
            string("terminal"),
            boolean("probeProcessLaunched"),
        ),
        (Some("dyld-open"), Some("open-entry-observed"), Some(true))
            | (
                Some("hook-generation-reader-catalog"),
                Some("reader-behavior-catalog-hit"),
                Some(false),
            )
            | (
                Some("state-home-reader-catalog" | "process-memory-reader-catalog"),
                Some("reader-behavior-cache-hit"),
                Some(false),
            )
    );
    string("permissionDecision") == Some("deny")
        && string("access") == Some("read")
        && string("accessMode") == Some("O_RDONLY")
        && boolean("cleanupVerified") == Some(true)
        && string("generationDigest") == Some(generation_digest)
        && trusted_reader_evidence
}

pub async fn validate_hook_binary_candidate(
    candidate: HookBinaryCandidateValidation<'_>,
) -> Result<(), String> {
    let hook_binary = candidate.hook_binary_path;
    #[cfg(target_os = "macos")]
    let (args, stdin) = {
        let fixture = crate::materialize_reader_probe_fixture()
            .map_err(|error| format!("materialize HookGeneration Reader fixture: {error}"))?;
        let subject = "asp-hook-candidate-validation.rs";
        let payload = serde_json::json!({
            "session_id": "hook-generation-candidate-validation",
            "cwd": candidate.state_home,
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": format!("{} read {subject}", fixture.display())
            }
        });
        let payload = serde_json::to_vec(&payload)
            .map_err(|error| format!("encode HookGeneration Reader validation payload: {error}"))?;
        (
            vec![
                "hook".to_owned(),
                "pre-tool".to_owned(),
                "--client".to_owned(),
                "codex".to_owned(),
                "--host-match".to_owned(),
                "Bash".to_owned(),
            ],
            Some((payload, subject.to_owned())),
        )
    };
    #[cfg(not(target_os = "macos"))]
    let (args, stdin): (Vec<String>, Option<(Vec<u8>, String)>) =
        (vec!["--version".to_owned()], None);

    let mut command = tokio::process::Command::new(hook_binary);
    let generation_root = candidate
        .generation_path
        .parent()
        .ok_or_else(|| "HookGeneration candidate has no generation root".to_owned())?;
    command
        .args(&args)
        .env("ASP_HOOK_GENERATION_ROOT", generation_root)
        .env("ASP_HOOK_GENERATION_DIGEST", candidate.generation_digest)
        .env("ASP_STATE_HOME", candidate.state_home)
        .env_remove("ASP_NO_AGENT")
        .kill_on_drop(true)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command.as_std_mut(), 0);
    let mut child = command.spawn().map_err(|error| {
        format!(
            "spawn HookGeneration binary candidate {}: {error}",
            hook_binary.display()
        )
    })?;
    if let Some((payload, _)) = &stdin {
        let mut child_stdin = child.stdin.take().ok_or_else(|| {
            format!(
                "HookGeneration binary candidate {} did not expose stdin",
                hook_binary.display()
            )
        })?;
        child_stdin.write_all(payload).await.map_err(|error| {
            format!(
                "write HookGeneration binary candidate {} stdin: {error}",
                hook_binary.display()
            )
        })?;
        child_stdin.shutdown().await.map_err(|error| {
            format!(
                "close HookGeneration binary candidate {} stdin: {error}",
                hook_binary.display()
            )
        })?;
    }

    let mut stdout = child.stdout.take().ok_or_else(|| {
        format!(
            "HookGeneration binary candidate {} did not expose stdout",
            hook_binary.display()
        )
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| {
        format!(
            "HookGeneration binary candidate {} did not expose stderr",
            hook_binary.display()
        )
    })?;
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await.map(|_| bytes)
    });

    let status = match tokio::time::timeout(VALIDATION_TIMEOUT, child.wait()).await {
        Ok(result) => result.map_err(|error| {
            format!(
                "wait for HookGeneration binary candidate {}: {error}",
                hook_binary.display()
            )
        })?,
        Err(_) => {
            #[cfg(unix)]
            let kill_result = {
                let process_group = child.id().ok_or_else(|| {
                    format!(
                        "HookGeneration binary candidate {} timed out without a process id",
                        hook_binary.display()
                    )
                })?;
                // SAFETY: the candidate is the leader of the isolated process group created
                // above. A negative pid targets only that group, including descendants that
                // otherwise keep the captured output pipes alive.
                let result = unsafe { libc::kill(-(process_group as i32), libc::SIGKILL) };
                if result == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            };
            #[cfg(not(unix))]
            let kill_result = child.kill().await;
            let reap_result = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(format!(
                "HookGeneration binary candidate {} exceeded {}ms validation timeout; process tree killed and reaped (kill={kill_result:?}, reap={reap_result:?})",
                hook_binary.display(),
                VALIDATION_TIMEOUT.as_millis()
            ));
        }
    };
    let stdout = stdout_task
        .await
        .map_err(|error| format!("join HookGeneration binary stdout reader: {error}"))?
        .map_err(|error| format!("read HookGeneration binary stdout: {error}"))?;
    let stderr = stderr_task
        .await
        .map_err(|error| format!("join HookGeneration binary stderr reader: {error}"))?
        .map_err(|error| format!("read HookGeneration binary stderr: {error}"))?;
    if !status.success() {
        return Err(format!(
            "HookGeneration binary candidate {} failed validation: status={status}; stderr={}",
            hook_binary.display(),
            String::from_utf8_lossy(&stderr)
        ));
    }
    if stdout.is_empty() {
        return Err(format!(
            "HookGeneration binary candidate {} produced empty validation output",
            hook_binary.display()
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let (_, registered_operand) = stdin.expect("macOS validation payload");
        let receipt: serde_json::Value = serde_json::from_slice(&stdout).map_err(|error| {
            format!(
                "decode HookGeneration binary candidate {} output: {error}",
                hook_binary.display()
            )
        })?;
        let hook_output = receipt.get("hookSpecificOutput").ok_or_else(|| {
            format!("candidate receipt is missing hookSpecificOutput: receipt={receipt}")
        })?;
        let context = hook_output
            .get("additionalContext")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "candidate receipt is missing additionalContext".to_owned())?;
        if hook_output
            .get("permissionDecision")
            .and_then(serde_json::Value::as_str)
            != Some("deny")
            || !context.contains(&registered_operand)
            || !valid_reader_validation_receipt(context, candidate.generation_digest)
        {
            return Err(format!(
                "HookGeneration binary candidate {} returned an invalid typed validation receipt",
                hook_binary.display()
            ));
        }
    }
    Ok(())
}
