use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;

const QUIESCENCE_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-artifact-quiescence";
const SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactQuiescenceLease {
    pub schema_id: String,
    pub schema_version: String,
    pub operation: String,
    pub producer_process_id: u32,
    pub lease_nonce: String,
    pub artifact_digest: Blake3ContentDigest,
    pub created_at_unix_millis: u128,
}

#[derive(Debug)]
pub struct PreparedRuntimeArtifactQuiescenceLease {
    pub lease: RuntimeArtifactQuiescenceLease,
    path: PathBuf,
    artifact_root: PathBuf,
}

impl PreparedRuntimeArtifactQuiescenceLease {
    pub fn consume_under_artifact_guard(
        &self,
        guard: &RuntimeArtifactMutationGuard,
    ) -> Result<PathBuf, String> {
        self.require_guard(guard)?;
        let consumed = self.path.with_file_name(format!(
            "quiescence-consumed-{}-{}.json",
            std::process::id(),
            self.lease.lease_nonce
        ));
        std::fs::rename(&self.path, &consumed).map_err(|error| {
            format!(
                "reasonKind=runtime-artifact-quiescence-consume-failed operation={} lease={} error={error}",
                self.lease.operation, self.lease.lease_nonce
            )
        })?;
        Ok(consumed)
    }

    pub fn restore_after_failed_commit(
        &self,
        consumed: &Path,
        guard: &RuntimeArtifactMutationGuard,
    ) -> Result<(), String> {
        self.require_guard(guard)?;
        std::fs::rename(consumed, &self.path).map_err(|error| {
            format!(
                "reasonKind=runtime-artifact-quiescence-restore-failed operation={} lease={} error={error}",
                self.lease.operation, self.lease.lease_nonce
            )
        })
    }

    pub fn finish_consumption(
        &self,
        consumed: &Path,
        guard: &RuntimeArtifactMutationGuard,
    ) -> Result<(), String> {
        self.require_guard(guard)?;
        std::fs::remove_file(consumed).map_err(|error| {
            format!(
                "reasonKind=runtime-artifact-quiescence-finalize-failed operation={} lease={} error={error}",
                self.lease.operation, self.lease.lease_nonce
            )
        })
    }

    fn require_guard(&self, guard: &RuntimeArtifactMutationGuard) -> Result<(), String> {
        if guard.admits(&self.artifact_root) {
            Ok(())
        } else {
            Err(
                "reasonKind=runtime-artifact-quiescence-guard-mismatch operation must use the preparing Artifact mutation guard"
                    .to_owned(),
            )
        }
    }
}

pub fn runtime_artifact_quiescence_lease_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("leases")
        .join("artifact-publication.v1.json")
}

pub(crate) fn prepare_runtime_artifact_quiescence_lease(
    state_home: &Path,
    operation: &str,
    artifact_digest: &Blake3ContentDigest,
    guard: &RuntimeArtifactMutationGuard,
) -> Result<PreparedRuntimeArtifactQuiescenceLease, String> {
    if operation.is_empty() {
        return Err(
            "reasonKind=runtime-artifact-quiescence-identity-incomplete operation or artifact digest is empty"
                .to_owned(),
        );
    }
    let artifact_root = state_home.join("runtime/artifacts");
    if !guard.admits(&artifact_root) {
        return Err(
            "reasonKind=runtime-artifact-quiescence-guard-mismatch lease recovery requires the canonical Artifact mutation guard"
                .to_owned(),
        );
    }
    let path = runtime_artifact_quiescence_lease_path(state_home);
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime artifact quiescence lease has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Runtime artifact lease directory: {error}"))?;
    if let Ok(bytes) = std::fs::read(&path) {
        let lease: RuntimeArtifactQuiescenceLease = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode Runtime artifact quiescence lease: {error}"))?;
        validate_lease_shape(&lease)?;
        let same_identity =
            lease.operation == operation && lease.artifact_digest == *artifact_digest;
        let same_producer = lease.producer_process_id == std::process::id();
        if producer_process_is_live_lease_owner(
            lease.producer_process_id,
            lease.created_at_unix_millis,
        ) && !(same_identity && same_producer)
        {
            return Err(format!(
                "reasonKind=runtime-artifact-quiescence-live-owner-conflict expectedOperation={operation} actualOperation={} expectedArtifactDigest={artifact_digest} actualArtifactDigest={} producerProcessId={}",
                lease.operation, lease.artifact_digest, lease.producer_process_id
            ));
        }
        if same_identity && same_producer {
            return Ok(PreparedRuntimeArtifactQuiescenceLease {
                lease,
                path,
                artifact_root,
            });
        }
    }

    let created_at_unix_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("read wall clock for quiescence lease: {error}"))?
        .as_millis();
    let producer_process_id = std::process::id();
    let nonce_material =
        format!("{operation}\0{artifact_digest}\0{producer_process_id}\0{created_at_unix_millis}");
    let lease = RuntimeArtifactQuiescenceLease {
        schema_id: QUIESCENCE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation: operation.to_owned(),
        producer_process_id,
        lease_nonce: blake3::hash(nonce_material.as_bytes()).to_hex().to_string(),
        artifact_digest: artifact_digest.clone(),
        created_at_unix_millis,
    };
    let temporary = path.with_file_name(format!(
        ".artifact-publication-{}-{}.tmp",
        producer_process_id, lease.lease_nonce
    ));
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&lease)
            .map_err(|error| format!("encode Runtime artifact quiescence lease: {error}"))?,
    )
    .map_err(|error| format!("stage Runtime artifact quiescence lease: {error}"))?;
    match std::fs::rename(&temporary, &path) {
        Ok(()) => Ok(PreparedRuntimeArtifactQuiescenceLease {
            lease,
            path,
            artifact_root,
        }),
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(format!(
                "reasonKind=runtime-artifact-quiescence-publication-failed operation={operation} error={error}"
            ))
        }
    }
}

#[cfg(unix)]
fn producer_process_exists(process_id: u32) -> bool {
    let Ok(process_id) = i32::try_from(process_id) else {
        return false;
    };
    if unsafe { libc::kill(process_id, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn producer_process_exists(process_id: u32) -> bool {
    // Non-Unix targets lack a portable PID probe. Conservatively preserve any
    // other producer and only admit an exact replay by this process.
    process_id != 0
}

fn producer_process_is_live_lease_owner(
    process_id: u32,
    lease_created_at_unix_millis: u128,
) -> bool {
    if !producer_process_exists(process_id) {
        return false;
    }
    match producer_process_started_at_unix_millis(process_id) {
        Some(started_at) => started_at <= lease_created_at_unix_millis,
        None => true,
    }
}

#[cfg(target_os = "macos")]
fn producer_process_started_at_unix_millis(process_id: u32) -> Option<u128> {
    let process_id = i32::try_from(process_id).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let expected = std::mem::size_of::<libc::proc_bsdinfo>();
    let expected_i32 = i32::try_from(expected).ok()?;
    let read = unsafe {
        libc::proc_pidinfo(
            process_id,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            expected_i32,
        )
    };
    if usize::try_from(read).ok()? != expected {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(
        u128::from(info.pbi_start_tvsec)
            .saturating_mul(1_000)
            .saturating_add(u128::from(info.pbi_start_tvusec) / 1_000),
    )
}

#[cfg(target_os = "linux")]
fn producer_process_started_at_unix_millis(process_id: u32) -> Option<u128> {
    let stat = std::fs::read_to_string(format!("/proc/{process_id}/stat")).ok()?;
    let after_command = stat.rsplit_once(") ")?.1;
    let start_ticks = after_command
        .split_whitespace()
        .nth(19)?
        .parse::<u128>()
        .ok()?;
    let boot_time_seconds = std::fs::read_to_string("/proc/stat")?
        .lines()
        .find_map(|line| line.strip_prefix("btime "))?
        .parse::<u128>()
        .ok()?;
    let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    let ticks_per_second = u128::try_from(ticks_per_second).ok()?;
    (ticks_per_second > 0).then(|| {
        boot_time_seconds
            .saturating_mul(1_000)
            .saturating_add(start_ticks.saturating_mul(1_000) / ticks_per_second)
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn producer_process_started_at_unix_millis(_: u32) -> Option<u128> {
    None
}

fn validate_lease_shape(lease: &RuntimeArtifactQuiescenceLease) -> Result<(), String> {
    if lease.schema_id != QUIESCENCE_SCHEMA_ID
        || lease.schema_version != SCHEMA_VERSION
        || lease.operation.is_empty()
        || lease.producer_process_id == 0
        || lease.lease_nonce.is_empty()
    {
        return Err(
            "reasonKind=runtime-artifact-quiescence-receipt-invalid stale lease is malformed"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_quiescence.rs"]
mod tests;
