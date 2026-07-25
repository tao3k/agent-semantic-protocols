use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ResolvedState;

const ACTIVITY_WRITE_INTERVAL_MS: u64 = 60 * 60 * 1_000;
const LAST_SEEN_FILE: &str = ".last-seen-ms";

/// Controls a project-registry garbage-collection pass.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistryGcOptions {
    pub apply: bool,
    pub grace_period_ms: u64,
}

impl Default for ProjectRegistryGcOptions {
    fn default() -> Self {
        Self {
            apply: false,
            grace_period_ms: 7 * 24 * 60 * 60 * 1_000,
        }
    }
}

/// One project directory evaluated by the registry collector.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectRegistryGcReason {
    CurrentRepository,
    RecordedCheckoutExists,
    NonCanonicalIdentityWithinGracePeriod,
    NonCanonicalRepositoryIdentity,
    MetadataMissingOrPartial,
    MissingCheckoutsWithinGracePeriod,
    AllRecordedCheckoutsMissing,
    RevalidationProtected,
    ActivityChangedDuringScan,
}

impl ProjectRegistryGcReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurrentRepository => "current-repository",
            Self::RecordedCheckoutExists => "recorded-checkout-exists",
            Self::NonCanonicalIdentityWithinGracePeriod => {
                "non-canonical-identity-within-grace-period"
            }
            Self::NonCanonicalRepositoryIdentity => "non-canonical-repository-identity",
            Self::MetadataMissingOrPartial => "metadata-missing-or-partial",
            Self::MissingCheckoutsWithinGracePeriod => "missing-checkouts-within-grace-period",
            Self::AllRecordedCheckoutsMissing => "all-recorded-checkouts-missing",
            Self::RevalidationProtected => "revalidation-protected",
            Self::ActivityChangedDuringScan => "activity-changed-during-scan",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectRegistryGcEligibility {
    Protected,
    Ineligible,
    Eligible,
}

impl ProjectRegistryGcEligibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Protected => "protected",
            Self::Ineligible => "ineligible",
            Self::Eligible => "eligible",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectRegistryGcDisposition {
    Retained,
    Removed,
}

impl ProjectRegistryGcDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retained => "retained",
            Self::Removed => "removed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRegistryGcMillis(u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistryGcCandidate {
    repo_id: super::identity::RepoId,
    project_dir: PathBuf,
    recorded_checkout_roots: Vec<PathBuf>,
    last_seen_ms: Option<ProjectRegistryGcMillis>,
    age_ms: Option<ProjectRegistryGcMillis>,
    reason: ProjectRegistryGcReason,
    eligibility: ProjectRegistryGcEligibility,
    disposition: ProjectRegistryGcDisposition,
}

impl ProjectRegistryGcCandidate {
    pub fn repo_id(&self) -> &super::identity::RepoId {
        &self.repo_id
    }

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    pub fn recorded_checkout_roots(&self) -> &[PathBuf] {
        &self.recorded_checkout_roots
    }

    pub fn last_seen_ms(&self) -> Option<u64> {
        self.last_seen_ms.map(|value| value.0)
    }

    pub fn age_ms(&self) -> Option<u64> {
        self.age_ms.map(|value| value.0)
    }

    pub const fn reason(&self) -> ProjectRegistryGcReason {
        self.reason
    }

    pub const fn eligibility(&self) -> ProjectRegistryGcEligibility {
        self.eligibility
    }

    pub const fn disposition(&self) -> ProjectRegistryGcDisposition {
        self.disposition
    }

    pub const fn protected(&self) -> bool {
        matches!(self.eligibility, ProjectRegistryGcEligibility::Protected)
    }

    pub const fn eligible(&self) -> bool {
        matches!(self.eligibility, ProjectRegistryGcEligibility::Eligible)
    }

    pub const fn removed(&self) -> bool {
        matches!(self.disposition, ProjectRegistryGcDisposition::Removed)
    }
}

/// Auditable result of a project-registry garbage-collection pass.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistryGcReport {
    pub state_home: PathBuf,
    pub project_registry_dir: PathBuf,
    pub apply: bool,
    pub grace_period_ms: u64,
    pub scanned_project_count: usize,
    pub candidate_count: usize,
    pub eligible_count: usize,
    pub removed_count: usize,
    pub candidates: Vec<ProjectRegistryGcCandidate>,
}

fn recorded_identity_is_noncanonical(
    state_home: &std::path::Path,
    roots: &[std::path::PathBuf],
    recorded_repo_id: &super::identity::RepoId,
) -> bool {
    let mut identities = roots
        .iter()
        .filter(|root| root.exists())
        .filter_map(|root| {
            ResolvedState::resolve_with_state_home(root, state_home)
                .ok()
                .map(|state| (state.repo.persistence.is_durable(), state.repo.repo_id))
        });
    let Some(first) = identities.next() else {
        return false;
    };
    identities.all(|identity| identity == first) && (!first.0 || &first.1 != recorded_repo_id)
}

impl ResolvedState {
    /// Record bounded project/workspace activity without rewriting identity metadata.
    pub(super) fn touch_registry_activity(&self) -> Result<(), String> {
        let now_ms = now_ms()?;
        touch_if_stale(
            &self.paths.project_dir.join(LAST_SEEN_FILE),
            now_ms,
            ACTIVITY_WRITE_INTERVAL_MS,
        )?;
        touch_if_stale(
            &self.paths.workspace_dir.join(LAST_SEEN_FILE),
            now_ms,
            ACTIVITY_WRITE_INTERVAL_MS,
        )
    }

    /// Scan or safely apply project-registry garbage collection.
    pub fn gc_project_registry(
        &self,
        options: ProjectRegistryGcOptions,
    ) -> Result<ProjectRegistryGcReport, String> {
        let now_ms = now_ms()?;
        let mut candidates = Vec::new();
        let mut scanned_project_count = 0usize;

        let project_registry_dir = &self.paths.projects_by_id_dir;
        let entries = match fs::read_dir(project_registry_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProjectRegistryGcReport {
                    state_home: self.state_home.clone(),
                    project_registry_dir: project_registry_dir.clone(),
                    apply: options.apply,
                    grace_period_ms: options.grace_period_ms,
                    scanned_project_count: 0,
                    candidate_count: 0,
                    eligible_count: 0,
                    removed_count: 0,
                    candidates,
                });
            }
            Err(error) => {
                return Err(format!(
                    "failed to scan project registry {}: {error}",
                    project_registry_dir.display()
                ));
            }
        };

        for entry in entries {
            let entry = entry.map_err(|error| format!("failed to read registry entry: {error}"))?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("failed to inspect registry entry: {error}"))?;
            if !file_type.is_dir() {
                continue;
            }
            let repo_id = super::identity::RepoId(entry.file_name().to_string_lossy().into_owned());
            if !repo_id.as_str().starts_with("repo-") {
                continue;
            }
            scanned_project_count += 1;
            let project_dir = entry.path();
            let roots = recorded_checkout_roots(&project_dir);
            let protected = repo_id == self.repo.repo_id;
            let all_roots_missing = roots.iter().all(|root| !root.exists());
            let non_canonical_identity =
                recorded_identity_is_noncanonical(&self.state_home, &roots, &repo_id);
            let last_seen_ms = last_seen_ms(&project_dir);
            let age_ms = last_seen_ms.map(|last_seen| now_ms.saturating_sub(last_seen));
            let old_enough = age_ms.is_some_and(|age| age >= options.grace_period_ms);
            let eligible =
                !protected && old_enough && (all_roots_missing || non_canonical_identity);
            let reason = if protected {
                ProjectRegistryGcReason::CurrentRepository
            } else if non_canonical_identity && !old_enough {
                ProjectRegistryGcReason::NonCanonicalIdentityWithinGracePeriod
            } else if non_canonical_identity {
                ProjectRegistryGcReason::NonCanonicalRepositoryIdentity
            } else if roots.iter().any(|root| root.exists()) {
                ProjectRegistryGcReason::RecordedCheckoutExists
            } else if roots.is_empty() {
                ProjectRegistryGcReason::MetadataMissingOrPartial
            } else if !old_enough {
                ProjectRegistryGcReason::MissingCheckoutsWithinGracePeriod
            } else {
                ProjectRegistryGcReason::AllRecordedCheckoutsMissing
            };
            let eligibility = if protected {
                ProjectRegistryGcEligibility::Protected
            } else if eligible {
                ProjectRegistryGcEligibility::Eligible
            } else {
                ProjectRegistryGcEligibility::Ineligible
            };

            if all_roots_missing || non_canonical_identity {
                candidates.push(ProjectRegistryGcCandidate {
                    repo_id,
                    project_dir,
                    recorded_checkout_roots: roots,
                    last_seen_ms: last_seen_ms.map(ProjectRegistryGcMillis),
                    age_ms: age_ms.map(ProjectRegistryGcMillis),
                    reason,
                    eligibility,
                    disposition: ProjectRegistryGcDisposition::Retained,
                });
            }
        }

        if options.apply {
            for candidate in &mut candidates {
                if !candidate.eligible() {
                    continue;
                }
                let roots = recorded_checkout_roots(&candidate.project_dir);
                let non_canonical_identity =
                    recorded_identity_is_noncanonical(&self.state_home, &roots, &candidate.repo_id);
                let identity_is_still_removable = match candidate.reason {
                    ProjectRegistryGcReason::NonCanonicalRepositoryIdentity => {
                        non_canonical_identity
                    }
                    _ => roots.iter().all(|root| !root.exists()),
                };
                if !identity_is_still_removable || candidate.repo_id == self.repo.repo_id {
                    candidate.eligibility = ProjectRegistryGcEligibility::Ineligible;
                    candidate.reason = ProjectRegistryGcReason::RevalidationProtected;
                    continue;
                }
                let last_seen = last_seen_ms(&candidate.project_dir);
                if last_seen != candidate.last_seen_ms() {
                    candidate.eligibility = ProjectRegistryGcEligibility::Ineligible;
                    candidate.reason = ProjectRegistryGcReason::ActivityChangedDuringScan;
                    continue;
                }
                fs::remove_dir_all(&candidate.project_dir).map_err(|error| {
                    format!(
                        "failed to remove derived project state {}: {error}",
                        candidate.project_dir.display()
                    )
                })?;
                candidate.disposition = ProjectRegistryGcDisposition::Removed;
            }
        }

        Ok(ProjectRegistryGcReport {
            state_home: self.state_home.clone(),
            project_registry_dir: project_registry_dir.clone(),
            apply: options.apply,
            grace_period_ms: options.grace_period_ms,
            scanned_project_count,
            candidate_count: candidates.len(),
            eligible_count: candidates
                .iter()
                .filter(|candidate| candidate.eligible())
                .count(),
            removed_count: candidates
                .iter()
                .filter(|candidate| candidate.removed())
                .count(),
            candidates,
        })
    }
}

fn touch_if_stale(path: &Path, now_ms: u64, interval_ms: u64) -> Result<(), String> {
    let previous = fs::read_to_string(path)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok());
    if previous.is_some_and(|value| now_ms.saturating_sub(value) < interval_ms) {
        return Ok(());
    }
    fs::write(path, format!("{now_ms}\n"))
        .map_err(|error| format!("failed to update activity {}: {error}", path.display()))
}

fn recorded_checkout_roots(project_dir: &Path) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    insert_json_path(
        &project_dir.join("project.json"),
        "checkoutRoot",
        &mut roots,
    );
    if let Ok(workspaces) = fs::read_dir(project_dir.join("workspaces")) {
        for workspace in workspaces.flatten() {
            if workspace.file_type().is_ok_and(|kind| kind.is_dir()) {
                insert_json_path(&workspace.path().join("workspace.json"), "root", &mut roots);
            }
        }
    }
    roots.into_iter().collect()
}

fn insert_json_path(path: &Path, key: &str, roots: &mut BTreeSet<PathBuf>) {
    let Ok(bytes) = fs::read(path) else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return;
    };
    if let Some(root) = value.get(key).and_then(Value::as_str) {
        roots.insert(PathBuf::from(root));
    }
}

fn last_seen_ms(project_dir: &Path) -> Option<u64> {
    let mut timestamps = Vec::new();
    if let Some(timestamp) = read_timestamp(&project_dir.join(LAST_SEEN_FILE)) {
        timestamps.push(timestamp);
    }
    if let Ok(workspaces) = fs::read_dir(project_dir.join("workspaces")) {
        for workspace in workspaces.flatten() {
            if let Some(timestamp) = read_timestamp(&workspace.path().join(LAST_SEEN_FILE)) {
                timestamps.push(timestamp);
            }
        }
    }
    timestamps
        .into_iter()
        .max()
        .or_else(|| modified_ms(project_dir))
}

fn read_timestamp(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn modified_ms(path: &Path) -> Option<u64> {
    path.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|_| "system clock milliseconds exceed u64".to_string())
        })
}
