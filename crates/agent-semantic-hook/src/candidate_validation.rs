use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug)]
pub struct HookEvaluatorCandidateValidation<'a> {
    pub evaluator_path: &'a Path,
    pub generation_path: &'a Path,
    pub generation_digest: &'a str,
    pub state_home: &'a Path,
}

pub async fn validate_hook_evaluator_candidate(
    candidate: HookEvaluatorCandidateValidation<'_>,
) -> Result<(), String> {
    let evaluator = candidate.evaluator_path;
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

    let mut command = tokio::process::Command::new(evaluator);
    command
        .args(&args)
        .env("ASP_HOOK_GENERATION_ROOT", candidate.generation_path)
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
            "spawn HookGeneration evaluator candidate {}: {error}",
            evaluator.display()
        )
    })?;
    if let Some((payload, _)) = &stdin {
        let mut child_stdin = child.stdin.take().ok_or_else(|| {
            format!(
                "HookGeneration evaluator candidate {} did not expose stdin",
                evaluator.display()
            )
        })?;
        child_stdin.write_all(payload).await.map_err(|error| {
            format!(
                "write HookGeneration evaluator candidate {} stdin: {error}",
                evaluator.display()
            )
        })?;
        child_stdin.shutdown().await.map_err(|error| {
            format!(
                "close HookGeneration evaluator candidate {} stdin: {error}",
                evaluator.display()
            )
        })?;
    }

    let mut stdout = child.stdout.take().ok_or_else(|| {
        format!(
            "HookGeneration evaluator candidate {} did not expose stdout",
            evaluator.display()
        )
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| {
        format!(
            "HookGeneration evaluator candidate {} did not expose stderr",
            evaluator.display()
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
                "wait for HookGeneration evaluator candidate {}: {error}",
                evaluator.display()
            )
        })?,
        Err(_) => {
            #[cfg(unix)]
            let kill_result = {
                let process_group = child.id().ok_or_else(|| {
                    format!(
                        "HookGeneration evaluator candidate {} timed out without a process id",
                        evaluator.display()
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
                "HookGeneration evaluator candidate {} exceeded {}ms validation timeout; process tree killed and reaped (kill={kill_result:?}, reap={reap_result:?})",
                evaluator.display(),
                VALIDATION_TIMEOUT.as_millis()
            ));
        }
    };
    let stdout = stdout_task
        .await
        .map_err(|error| format!("join HookGeneration evaluator stdout reader: {error}"))?
        .map_err(|error| format!("read HookGeneration evaluator stdout: {error}"))?;
    let stderr = stderr_task
        .await
        .map_err(|error| format!("join HookGeneration evaluator stderr reader: {error}"))?
        .map_err(|error| format!("read HookGeneration evaluator stderr: {error}"))?;
    if !status.success() {
        return Err(format!(
            "HookGeneration evaluator candidate {} failed validation: status={status}; stderr={}",
            evaluator.display(),
            String::from_utf8_lossy(&stderr)
        ));
    }
    if stdout.is_empty() {
        return Err(format!(
            "HookGeneration evaluator candidate {} produced empty validation output",
            evaluator.display()
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let (_, registered_operand) = stdin.expect("macOS validation payload");
        let receipt: serde_json::Value = serde_json::from_slice(&stdout).map_err(|error| {
            format!(
                "decode HookGeneration evaluator candidate {} output: {error}",
                evaluator.display()
            )
        })?;
        let hook_output = receipt
            .get("hookSpecificOutput")
            .ok_or_else(|| "candidate receipt is missing hookSpecificOutput".to_owned())?;
        let context = hook_output
            .get("additionalContext")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "candidate receipt is missing additionalContext".to_owned())?;
        let expected = registered_operand;
        if hook_output.get("permissionDecision").and_then(serde_json::Value::as_str)
            != Some("deny")
            || !context.contains(&expected)
            || !context.contains("\"access\":\"read\"")
            || !context.contains("\"accessMode\":\"O_RDONLY\"")
            || !context.contains("\"backend\":\"dyld-open\"")
            || !context.contains("\"terminal\":\"open-entry-observed\"")
            || !context.contains("\"probeProcessLaunched\":true")
            || !context.contains("\"cleanupVerified\":true")
            || !context.contains(candidate.generation_digest)
        {
            return Err(format!(
                "HookGeneration evaluator candidate {} returned an invalid typed validation receipt",
                evaluator.display()
            ));
        }
    }
    Ok(())
}
