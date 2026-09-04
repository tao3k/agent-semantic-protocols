//! Output collection, process-group cleanup, and memory observation.

use std::io::ErrorKind;
use std::process::ExitStatus;
use std::time::Duration;
use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::warn;

use crate::capture::LimitedRead;
use crate::process_contract::ProviderProcessError;
use crate::process_contract::ProviderProcessLimits;
use crate::process_contract::ProviderProcessReceipt;
use crate::process_contract::StdinMode;

use super::ProviderIoTasks;
use super::runtime::ProviderChild;
use super::runtime::ProviderProcessOutput;

const PROVIDER_MEMORY_OBSERVATION_GRACE: Duration = Duration::from_millis(250);
const PROVIDER_MEMORY_OBSERVATION_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) async fn collect_provider_output(
    mut child: ProviderChild,
    tasks: ProviderIoTasks,
    limits: ProviderProcessLimits,
    start: Instant,
    admission_wait: Duration,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    let ProviderIoTasks {
        stdin: stdin_task,
        stdout: stdout_task,
        stderr: stderr_task,
    } = tasks;
    let child_pid = child.id();
    debug!(
        child_pid,
        memory_limit_bytes = limits.memory_limit_bytes(),
        "provider memory gate armed"
    );

    let status = if let Some(timeout) = limits.timeout() {
        tokio::select! {
            biased;
            result = child.wait() => {
                result.map_err(|source| ProviderProcessError::Wait { source })?
            }
            _ = tokio::time::sleep(timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|source| ProviderProcessError::Wait { source })?
                {
                    status
                } else {
                warn!(
                    timeout_ms = timeout.as_millis(),
                    "provider process timed out; requesting kill"
                );
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) =
                    join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::Timeout {
                    timeout,
                    receipt: Box::new(provider_process_receipt(
                        None,
                        &ProviderProcessCompletion {
                            start,
                            admission_wait,
                            stdout,
                            stderr,
                            timed_out: true,
                            memory_limit_exceeded: false,
                            descendant_cleanup_required: false,
                            limits,
                        },
                    )),
                });
                }
            }
            observation = provider_memory_limit_exceeded(child_pid, limits.memory_limit_bytes()) => {
                let limit_bytes = limits.memory_limit_bytes().expect("memory monitor requires limit");
                warn!(
                    limit_bytes,
                    observed_resident_bytes = observation.resident_bytes,
                    observed_process_count = observation.process_count,
                    termination_action = "kill-process-group",
                    "provider process exceeded memory limit; requesting kill"
                );
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) = join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::MemoryLimit {
                    limit_bytes,
                    receipt: Box::new(provider_process_receipt(
                        None,
                        &ProviderProcessCompletion {
                            start,
                            admission_wait,
                            stdout,
                            stderr,
                            timed_out: false,
                            memory_limit_exceeded: true,
                            descendant_cleanup_required: false,
                            limits,
                        },
                    )),
                });
            }
        }
    } else {
        tokio::select! {
            biased;
            result = child.wait() => {
                result.map_err(|source| ProviderProcessError::Wait { source })?
            }
            observation = provider_memory_limit_exceeded(child_pid, limits.memory_limit_bytes()) => {
                let limit_bytes = limits.memory_limit_bytes().expect("memory monitor requires limit");
                warn!(
                    limit_bytes,
                    observed_resident_bytes = observation.resident_bytes,
                    observed_process_count = observation.process_count,
                    termination_action = "kill-process-group",
                    "provider process exceeded memory limit; requesting kill"
                );
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) = join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::MemoryLimit {
                    limit_bytes,
                    receipt: Box::new(provider_process_receipt(
                        None,
                        &ProviderProcessCompletion {
                            start,
                            admission_wait,
                            stdout,
                            stderr,
                            timed_out: false,
                            memory_limit_exceeded: true,
                            descendant_cleanup_required: false,
                            limits,
                        },
                    )),
                });
            }
        }
    };

    // Provider invocations are not resident. Once the leader exits, any
    // remaining descendant belongs to this invocation and must be terminated
    // before inherited output pipes can keep collection alive indefinitely.
    // Process-group membership is inherited across fork, so once the leader
    // has exited there is no userspace publication step to wait for. Signal
    // the group directly: probing with signal 0 before SIGKILL creates a
    // needless TOCTOU window and used to discard the actual SIGKILL result.
    let descendant_cleanup_required = kill_provider_process_group(child.process_group_id);
    if descendant_cleanup_required {
        warn!(
            process_group_id = child.process_group_id,
            termination_action = "kill-process-group",
            descendant_cleanup_required = true,
            "one-shot provider leader exited with live descendants; terminated orphan process group"
        );
    }

    debug!(
        status = ?status.code(),
        elapsed_ms = start.elapsed().as_millis(),
        "provider process exited"
    );
    join_transport_task(stdin_task, "stdin").await?;
    let stdout = join_transport_task(stdout_task, "stdout").await?;
    let stderr = join_transport_task(stderr_task, "stderr").await?;
    Ok(provider_process_output(
        status,
        ProviderProcessCompletion {
            start,
            admission_wait,
            stdout,
            stderr,
            timed_out: false,
            memory_limit_exceeded: false,
            descendant_cleanup_required,
            limits,
        },
    ))
}

async fn terminate_provider_process(child: &mut ProviderChild) {
    kill_provider_process_group(child.process_group_id);
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(unix)]
pub(super) fn provider_process_group_id(child: &Child) -> Option<i32> {
    child.id().and_then(|pid| i32::try_from(pid).ok())
}

#[cfg(not(unix))]
pub(super) fn provider_process_group_id(_child: &Child) -> Option<i32> {
    None
}

#[cfg(unix)]
pub(super) fn kill_provider_process_group(process_group_id: Option<i32>) -> bool {
    let Some(process_group_id) = process_group_id else {
        return false;
    };
    loop {
        if unsafe { libc::kill(-process_group_id, libc::SIGKILL) } == 0 {
            return true;
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == ErrorKind::Interrupted {
            continue;
        }
        if error.raw_os_error() != Some(libc::ESRCH) {
            warn!(
                process_group_id,
                error = %error,
                termination_action = "kill-process-group",
                "failed to terminate provider process group"
            );
        }
        return false;
    }
}

#[cfg(not(unix))]
pub(super) fn kill_provider_process_group(_process_group_id: Option<i32>) -> bool {
    false
}

struct ProviderProcessCompletion {
    start: Instant,
    admission_wait: Duration,
    stdout: LimitedRead,
    stderr: LimitedRead,
    timed_out: bool,
    memory_limit_exceeded: bool,
    descendant_cleanup_required: bool,
    limits: ProviderProcessLimits,
}

fn provider_process_output(
    status: ExitStatus,
    completion: ProviderProcessCompletion,
) -> ProviderProcessOutput {
    let receipt = provider_process_receipt(Some(&status), &completion);
    ProviderProcessOutput {
        status,
        receipt,
        stdout: completion.stdout.bytes,
        stderr: completion.stderr.bytes,
    }
}

fn provider_process_receipt(
    status: Option<&ExitStatus>,
    completion: &ProviderProcessCompletion,
) -> ProviderProcessReceipt {
    let ProviderProcessCompletion {
        start,
        admission_wait,
        stdout,
        stderr,
        timed_out,
        memory_limit_exceeded,
        descendant_cleanup_required,
        limits,
    } = completion;
    let exit_signal = provider_exit_signal(status);
    let status_success = status.is_some_and(|status| status.success());
    let memory_limit_enforced = cfg!(unix) && limits.memory_limit_bytes().is_some();
    let memory_limit_suspected =
        memory_limit_enforced && exit_signal.is_some_and(memory_limit_failure_signal);
    let termination_reason = if *memory_limit_exceeded {
        "memory-limit-exceeded"
    } else if *timed_out {
        "timeout"
    } else if memory_limit_suspected {
        "memory-limit-suspected"
    } else if exit_signal.is_some() {
        "signal"
    } else if status_success {
        "success"
    } else {
        "exit-code"
    };
    ProviderProcessReceipt::from_input(crate::process_contract::ProviderProcessReceiptInput {
        elapsed: start.elapsed(),
        admission_wait: *admission_wait,
        status_code: status.and_then(ExitStatus::code),
        status_success,
        stdout_bytes: stdout.total_bytes,
        stderr_bytes: stderr.total_bytes,
        stdout_sha256: stdout.sha256.clone(),
        stderr_sha256: stderr.sha256.clone(),
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        timed_out: *timed_out,
        memory_limit_exceeded: *memory_limit_exceeded,
        exit_signal,
        memory_limit_bytes: limits.memory_limit_bytes(),
        memory_limit_enforced,
        process_group_isolation_enforced: cfg!(unix),
        descendant_cleanup_required: *descendant_cleanup_required,
        abnormal_termination: *timed_out || *memory_limit_exceeded || !status_success,
        termination_reason: termination_reason.to_string(),
    })
}

#[derive(Clone, Copy, Debug)]
struct ProviderMemoryObservation {
    resident_bytes: u64,
    process_count: usize,
}

#[cfg(target_os = "macos")]
async fn provider_memory_limit_exceeded(
    pid: Option<u32>,
    limit: Option<u64>,
) -> ProviderMemoryObservation {
    let (Some(pid), Some(limit)) = (pid, limit) else {
        return std::future::pending::<ProviderMemoryObservation>().await;
    };
    tokio::time::sleep(PROVIDER_MEMORY_OBSERVATION_GRACE).await;
    loop {
        if let Some(observation) = macos_process_group_memory(pid)
            && observation.resident_bytes > limit
        {
            return observation;
        }
        tokio::time::sleep(PROVIDER_MEMORY_OBSERVATION_POLL_INTERVAL).await;
    }
}

#[cfg(not(target_os = "macos"))]
async fn provider_memory_limit_exceeded(
    _pid: Option<u32>,
    _limit: Option<u64>,
) -> ProviderMemoryObservation {
    std::future::pending::<ProviderMemoryObservation>().await
}

#[cfg(target_os = "macos")]
fn macos_process_group_memory(process_group_id: u32) -> Option<ProviderMemoryObservation> {
    unsafe extern "C" {
        fn proc_listallpids(buffer: *mut libc::c_void, buffersize: i32) -> i32;
    }
    let pid_count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    let capacity = usize::try_from(pid_count).ok()?.saturating_add(64);
    let mut pids = vec![0_i32; capacity];
    let buffer_bytes = i32::try_from(pids.len().checked_mul(std::mem::size_of::<i32>())?).ok()?;
    let listed = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), buffer_bytes) };
    let listed = usize::try_from(listed).ok()?.min(pids.len());
    let process_group_id = i32::try_from(process_group_id).ok()?;
    let mut resident_bytes = 0_u64;
    let mut process_count = 0_usize;
    for pid in pids.into_iter().take(listed).filter(|pid| *pid > 0) {
        if unsafe { libc::getpgid(pid) } != process_group_id {
            continue;
        }
        let Some(process_resident_bytes) = macos_resident_bytes(pid as u32) else {
            continue;
        };
        resident_bytes = resident_bytes.saturating_add(process_resident_bytes);
        process_count += 1;
    }
    (process_count > 0).then_some(ProviderMemoryObservation {
        resident_bytes,
        process_count,
    })
}

#[cfg(target_os = "macos")]
fn macos_resident_bytes(pid: u32) -> Option<u64> {
    #[repr(C)]
    #[derive(Default)]
    struct ProcTaskInfo {
        virtual_size: u64,
        resident_size: u64,
        total_user: u64,
        total_system: u64,
        threads_user: u64,
        threads_system: u64,
        policy: i32,
        faults: i32,
        pageins: i32,
        cow_faults: i32,
        messages_sent: i32,
        messages_received: i32,
        syscalls_mach: i32,
        syscalls_unix: i32,
        csw: i32,
        threadnum: i32,
        numrunning: i32,
        priority: i32,
    }
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut libc::c_void,
            buffersize: i32,
        ) -> i32;
    }
    const PROC_PIDTASKINFO: i32 = 4;
    let mut info = ProcTaskInfo::default();
    let size = std::mem::size_of::<ProcTaskInfo>() as i32;
    let read = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDTASKINFO,
            0,
            (&mut info as *mut ProcTaskInfo).cast(),
            size,
        )
    };
    (read == size).then_some(info.resident_size)
}

#[cfg(unix)]
fn provider_exit_signal(status: Option<&ExitStatus>) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.and_then(ExitStatusExt::signal)
}

#[cfg(not(unix))]
fn provider_exit_signal(_status: Option<&ExitStatus>) -> Option<i32> {
    None
}

#[cfg(unix)]
fn memory_limit_failure_signal(signal: i32) -> bool {
    matches!(
        signal,
        libc::SIGKILL | libc::SIGSEGV | libc::SIGABRT | libc::SIGBUS
    )
}

#[cfg(not(unix))]
const fn memory_limit_failure_signal(_: i32) -> bool {
    false
}

pub(super) async fn write_stdin(
    stdin: Option<ChildStdin>,
    stdin_mode: StdinMode,
) -> Result<(), ProviderProcessError> {
    if let StdinMode::Bytes(bytes) = stdin_mode {
        let mut stdin = stdin.ok_or(ProviderProcessError::CaptureStdin)?;
        if let Err(source) = stdin.write_all(&bytes).await {
            if source.kind() == ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(ProviderProcessError::StdinWrite { source });
        }
        if let Err(source) = stdin.shutdown().await {
            if source.kind() == ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(ProviderProcessError::StdinClose { source });
        }
    }
    Ok(())
}

async fn join_transport_task<T>(
    task: JoinHandle<Result<T, ProviderProcessError>>,
    task_name: &'static str,
) -> Result<T, ProviderProcessError> {
    task.await.map_err(|source| ProviderProcessError::Join {
        task: task_name,
        source,
    })?
}

async fn join_readers_after_timeout(
    stdout_task: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
    stderr_task: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
) -> (LimitedRead, LimitedRead) {
    let stdout = match join_transport_task(stdout_task, "stdout").await {
        Ok(stdout) => stdout,
        Err(_) => LimitedRead::empty(),
    };
    let stderr = match join_transport_task(stderr_task, "stderr").await {
        Ok(stderr) => stderr,
        Err(_) => LimitedRead::empty(),
    };
    (stdout, stderr)
}
