//! Runtime-owned target coverage for cold query admission generations.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::AdmissionEntry;

#[derive(Default)]
pub(super) struct QueryTargetCoverage {
    building: BTreeSet<PathBuf>,
    building_full: bool,
    ready: BTreeSet<PathBuf>,
    ready_full: bool,
}

impl AdmissionEntry {
    pub(super) fn ready_covers(&self, requested: &BTreeSet<PathBuf>) -> bool {
        self.query_target_coverage.lock().is_ok_and(|coverage| {
            coverage.ready_full || requested.iter().all(|path| coverage.ready.contains(path))
        })
    }

    pub(super) fn building_covers(&self, requested: &BTreeSet<PathBuf>) -> bool {
        self.query_target_coverage.lock().is_ok_and(|coverage| {
            coverage.building_full
                || requested
                    .iter()
                    .all(|path| coverage.building.contains(path))
        })
    }

    pub(super) fn begin_query_targets(&self, requested: &BTreeSet<PathBuf>) -> Result<(), String> {
        let mut coverage = self
            .query_target_coverage
            .lock()
            .map_err(|_| "workspace query target coverage lock poisoned".to_owned())?;
        coverage.building = requested.clone();
        coverage.building_full = requested.is_empty();
        Ok(())
    }

    pub(super) fn complete_query_targets(&self, ready: bool) -> Result<(), String> {
        let mut coverage = self
            .query_target_coverage
            .lock()
            .map_err(|_| "workspace query target coverage lock poisoned".to_owned())?;
        if ready {
            if coverage.building_full {
                coverage.ready_full = true;
            }
            let built = std::mem::take(&mut coverage.building);
            coverage.ready.extend(built);
        } else {
            coverage.building.clear();
        }
        coverage.building_full = false;
        Ok(())
    }
}
