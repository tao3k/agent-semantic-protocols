//! Cross-process operation locks for local Turso client DB writers.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use fs2::FileExt;

use super::turso_lock_policy::{
    TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_ATTEMPTS, TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_MS,
};

pub(super) struct TursoOperationLockGuard {
    path: PathBuf,
    _file: Option<File>,
}

thread_local! {
    static HELD_TURSO_OPERATION_LOCKS: RefCell<BTreeMap<PathBuf, usize>> =
        const { RefCell::new(BTreeMap::new()) };
}

pub(super) fn acquire_turso_operation_lock(
    db_path: &Path,
    operation: &str,
) -> Result<TursoOperationLockGuard, String> {
    let lock_path = turso_operation_lock_path(db_path)?;
    if increment_held_lock_if_present(&lock_path) {
        return Ok(TursoOperationLockGuard {
            path: lock_path,
            _file: None,
        });
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "failed to open Turso operation lock {} for {operation}: {error}",
                lock_path.display()
            )
        })?;

    for attempt in 0..TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_ATTEMPTS {
        match file.try_lock_exclusive() {
            Ok(()) => {
                insert_held_lock(&lock_path);
                return Ok(TursoOperationLockGuard {
                    path: lock_path,
                    _file: Some(file),
                });
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if attempt + 1 == TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_ATTEMPTS {
                    return Err(format!(
                        "timed out waiting for Turso operation lock {} for {operation}",
                        lock_path.display()
                    ));
                }
                thread::sleep(Duration::from_millis(
                    TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_MS,
                ));
            }
            Err(error) => {
                return Err(format!(
                    "failed to lock Turso operation lock {} for {operation}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
    Err(format!(
        "timed out waiting for Turso operation lock {} for {operation}",
        lock_path.display()
    ))
}

impl Drop for TursoOperationLockGuard {
    fn drop(&mut self) {
        decrement_held_lock(&self.path);
    }
}

fn increment_held_lock_if_present(lock_path: &Path) -> bool {
    HELD_TURSO_OPERATION_LOCKS.with(|locks| {
        let mut locks = locks.borrow_mut();
        let Some(count) = locks.get_mut(lock_path) else {
            return false;
        };
        *count += 1;
        true
    })
}

fn insert_held_lock(lock_path: &Path) {
    HELD_TURSO_OPERATION_LOCKS.with(|locks| {
        locks.borrow_mut().insert(lock_path.to_path_buf(), 1);
    });
}

fn decrement_held_lock(lock_path: &Path) {
    HELD_TURSO_OPERATION_LOCKS.with(|locks| {
        let mut locks = locks.borrow_mut();
        if let Some(count) = locks.get_mut(lock_path)
            && *count > 1
        {
            *count -= 1;
            return;
        }
        locks.remove(lock_path);
    });
}

fn turso_operation_lock_path(db_path: &Path) -> Result<PathBuf, String> {
    let file_name = db_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .ok_or_else(|| format!("Turso DB path has no file name: {}", db_path.display()))?;
    Ok(db_path.with_file_name(format!("{file_name}.operation.lock")))
}
