//! Compiles `agentOrgArtifacts` hook policy and caches filesystem-derived Org artifact state.

use agent_semantic_config::{
    HookClientAgentOrgArtifactsArchiveWarningConfig, HookClientAgentOrgArtifactsConfig,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ARCHIVE_WARNING_CACHE_TTL: Duration = Duration::from_secs(5);
const RECOVERY_ACTIVE_CACHE_TTL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub(crate) struct AgentOrgArtifactsRecovery {
    pub(crate) artifacts_path: String,
    pub(crate) entry_skill_path: String,
    pub(crate) inactive_after_minutes: u64,
    pub(crate) capture_contract_command: String,
}

/// Archive warning metadata rendered into hook decisions for active Org artifacts.
#[derive(Debug, Clone)]
pub struct AgentOrgArtifactsArchiveWarning {
    pub artifacts_path: String,
    pub archives_dir: String,
    pub active_org_file_threshold: usize,
    pub active_org_file_count: usize,
    pub done_org_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledAgentOrgArtifactsConfig {
    enabled: bool,
    inactive_after_minutes: u64,
    artifacts_path: PathBuf,
    entry_skill_path: String,
    archive_warning: CompiledAgentOrgArtifactsArchiveWarningConfig,
}

#[derive(Debug, Clone)]
struct CompiledAgentOrgArtifactsArchiveWarningConfig {
    enabled: bool,
    active_org_file_threshold: usize,
    archives_dir: String,
    max_reported_files: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ArchiveWarningCacheKey {
    artifacts_root: PathBuf,
    archives_dir: String,
    active_org_file_threshold: usize,
    max_reported_files: usize,
}

#[derive(Clone)]
struct ArchiveWarningCacheEntry {
    checked_at: Instant,
    warning: Option<AgentOrgArtifactsArchiveWarning>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RecoveryActiveCacheKey {
    artifacts_root: PathBuf,
    inactive_after_secs: u64,
}

#[derive(Clone)]
struct RecoveryActiveCacheEntry {
    checked_at: Instant,
    active: bool,
}

static ARCHIVE_WARNING_CACHE: OnceLock<
    Mutex<HashMap<ArchiveWarningCacheKey, ArchiveWarningCacheEntry>>,
> = OnceLock::new();
static RECOVERY_ACTIVE_CACHE: OnceLock<
    Mutex<HashMap<RecoveryActiveCacheKey, RecoveryActiveCacheEntry>>,
> = OnceLock::new();

impl Default for CompiledAgentOrgArtifactsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            inactive_after_minutes: 30,
            artifacts_path: PathBuf::new(),
            entry_skill_path: String::new(),
            archive_warning: CompiledAgentOrgArtifactsArchiveWarningConfig::default(),
        }
    }
}

impl Default for CompiledAgentOrgArtifactsArchiveWarningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            active_org_file_threshold: 10,
            archives_dir: "archives".to_string(),
            max_reported_files: 5,
        }
    }
}

impl CompiledAgentOrgArtifactsConfig {
    pub(crate) fn disabled() -> Self {
        Self::default()
    }

    pub(crate) fn recovery(
        &self,
        project_root: &Path,
        session_id: Option<&str>,
    ) -> Option<AgentOrgArtifactsRecovery> {
        if !self.enabled {
            return None;
        }
        let artifacts_root = resolve_project_path(project_root, &self.artifacts_path);
        let inactive_after_secs = self.inactive_after_minutes.saturating_mul(60);
        let inactive_after = Duration::from_secs(inactive_after_secs);
        if cached_agent_org_contract_artifacts_active(&artifacts_root, inactive_after_secs)
            .unwrap_or_else(|| {
                let active = agent_org_contract_artifacts_active(&artifacts_root, inactive_after);
                store_agent_org_contract_artifacts_active(
                    RecoveryActiveCacheKey {
                        artifacts_root: artifacts_root.clone(),
                        inactive_after_secs,
                    },
                    active,
                );
                active
            })
        {
            return None;
        }
        Some(AgentOrgArtifactsRecovery {
            artifacts_path: artifacts_root.display().to_string(),
            entry_skill_path: self.entry_skill_path.clone(),
            inactive_after_minutes: self.inactive_after_minutes,
            capture_contract_command: capture_contract_command(&artifacts_root, session_id),
        })
    }

    pub(crate) fn archive_warning(
        &self,
        project_root: &Path,
    ) -> Option<AgentOrgArtifactsArchiveWarning> {
        if !self.enabled || !self.archive_warning.enabled {
            return None;
        }
        let artifacts_root = resolve_project_path(project_root, &self.artifacts_path);
        let cache_key = ArchiveWarningCacheKey {
            artifacts_root: artifacts_root.clone(),
            archives_dir: self.archive_warning.archives_dir.clone(),
            active_org_file_threshold: self.archive_warning.active_org_file_threshold,
            max_reported_files: self.archive_warning.max_reported_files,
        };
        if let Some(cached) = cached_archive_warning(&cache_key) {
            return cached;
        }
        let warning = self.compute_archive_warning(&artifacts_root);
        store_archive_warning(cache_key, warning.clone());
        warning
    }

    fn compute_archive_warning(
        &self,
        artifacts_root: &Path,
    ) -> Option<AgentOrgArtifactsArchiveWarning> {
        let active_org_files =
            collect_active_org_files(artifacts_root, &self.archive_warning.archives_dir);
        if active_org_files.len() <= self.archive_warning.active_org_file_threshold {
            return None;
        }
        let done_org_files = active_org_files
            .iter()
            .filter(|path| org_file_has_done_heading(path))
            .take(self.archive_warning.max_reported_files)
            .map(|path| artifact_display_path(artifacts_root, path))
            .collect::<Vec<_>>();
        if done_org_files.is_empty() {
            return None;
        }
        Some(AgentOrgArtifactsArchiveWarning {
            artifacts_path: artifacts_root.display().to_string(),
            archives_dir: self.archive_warning.archives_dir.clone(),
            active_org_file_threshold: self.archive_warning.active_org_file_threshold,
            active_org_file_count: active_org_files.len(),
            done_org_files,
        })
    }
}

fn archive_warning_cache()
-> &'static Mutex<HashMap<ArchiveWarningCacheKey, ArchiveWarningCacheEntry>> {
    ARCHIVE_WARNING_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_archive_warning(
    key: &ArchiveWarningCacheKey,
) -> Option<Option<AgentOrgArtifactsArchiveWarning>> {
    let cache = archive_warning_cache().lock().ok()?;
    let entry = cache.get(key)?;
    if entry.checked_at.elapsed() <= ARCHIVE_WARNING_CACHE_TTL {
        return Some(entry.warning.clone());
    }
    None
}

fn store_archive_warning(
    key: ArchiveWarningCacheKey,
    warning: Option<AgentOrgArtifactsArchiveWarning>,
) {
    if let Ok(mut cache) = archive_warning_cache().lock() {
        cache.insert(
            key,
            ArchiveWarningCacheEntry {
                checked_at: Instant::now(),
                warning,
            },
        );
    }
}

fn recovery_active_cache()
-> &'static Mutex<HashMap<RecoveryActiveCacheKey, RecoveryActiveCacheEntry>> {
    RECOVERY_ACTIVE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_agent_org_contract_artifacts_active(
    artifacts_root: &Path,
    inactive_after_secs: u64,
) -> Option<bool> {
    let cache = recovery_active_cache().lock().ok()?;
    let entry = cache.get(&RecoveryActiveCacheKey {
        artifacts_root: artifacts_root.to_path_buf(),
        inactive_after_secs,
    })?;
    if entry.checked_at.elapsed() <= RECOVERY_ACTIVE_CACHE_TTL {
        return Some(entry.active);
    }
    None
}

fn store_agent_org_contract_artifacts_active(key: RecoveryActiveCacheKey, active: bool) {
    if let Ok(mut cache) = recovery_active_cache().lock() {
        cache.insert(
            key,
            RecoveryActiveCacheEntry {
                checked_at: Instant::now(),
                active,
            },
        );
    }
}

impl TryFrom<HookClientAgentOrgArtifactsConfig> for CompiledAgentOrgArtifactsConfig {
    type Error = String;

    fn try_from(config: HookClientAgentOrgArtifactsConfig) -> Result<Self, Self::Error> {
        if config.inactive_after_minutes() == 0 {
            return Err(
                "agentOrgArtifacts.inactiveAfterMinutes must be greater than 0".to_string(),
            );
        }
        if config.artifacts_path().is_empty() {
            return Err("agentOrgArtifacts.artifactsPath must not be empty".to_string());
        }
        if config.entry_skill_path().is_empty() {
            return Err("agentOrgArtifacts.entrySkillPath must not be empty".to_string());
        }
        Ok(Self {
            enabled: config.enabled(),
            inactive_after_minutes: config.inactive_after_minutes(),
            artifacts_path: PathBuf::from(config.artifacts_path()),
            entry_skill_path: config.entry_skill_path().to_owned(),
            archive_warning: CompiledAgentOrgArtifactsArchiveWarningConfig::try_from(
                config.archive_warning(),
            )?,
        })
    }
}

impl TryFrom<&HookClientAgentOrgArtifactsArchiveWarningConfig>
    for CompiledAgentOrgArtifactsArchiveWarningConfig
{
    type Error = String;

    fn try_from(
        config: &HookClientAgentOrgArtifactsArchiveWarningConfig,
    ) -> Result<Self, Self::Error> {
        if config.active_org_file_threshold == 0 {
            return Err(
                "agentOrgArtifacts.archiveWarning.activeOrgFileThreshold must be greater than 0"
                    .to_string(),
            );
        }
        if config.max_reported_files == 0 {
            return Err(
                "agentOrgArtifacts.archiveWarning.maxReportedFiles must be greater than 0"
                    .to_string(),
            );
        }
        if config.archives_dir.is_empty() {
            return Err(
                "agentOrgArtifacts.archiveWarning.archivesDir must not be empty".to_string(),
            );
        }
        Ok(Self {
            enabled: config.enabled,
            active_org_file_threshold: config.active_org_file_threshold,
            archives_dir: config.archives_dir.clone(),
            max_reported_files: config.max_reported_files,
        })
    }
}

fn resolve_project_path(project_root: &Path, configured_path: &Path) -> PathBuf {
    if configured_path.is_absolute() {
        configured_path.to_path_buf()
    } else {
        project_root.join(configured_path)
    }
}

fn agent_org_contract_artifacts_active(root: &Path, inactive_after: Duration) -> bool {
    let Some(modified) = latest_contract_org_artifact_modified(root, inactive_after) else {
        return false;
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age <= inactive_after,
        Err(_) => true,
    }
}

fn latest_contract_org_artifact_modified(
    root: &Path,
    inactive_after: Duration,
) -> Option<SystemTime> {
    let mut latest = None;
    collect_latest_contract_org_artifact_modified(root, inactive_after, &mut latest);
    latest
}

fn collect_latest_contract_org_artifact_modified(
    root: &Path,
    inactive_after: Duration,
    latest: &mut Option<SystemTime>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if should_descend_org_artifact_dir(&entry.file_name().to_string_lossy()) {
                collect_latest_contract_org_artifact_modified(&path, inactive_after, latest);
            }
            continue;
        }
        if !file_type.is_file() || !is_org_file_path(&path) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) else {
            continue;
        };
        if !modified_is_recent(modified, inactive_after) || !org_file_has_contract_binding(&path) {
            continue;
        }
        if latest.is_none_or(|current| modified > current) {
            *latest = Some(modified);
        }
    }
}

fn modified_is_recent(modified: SystemTime, inactive_after: Duration) -> bool {
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age <= inactive_after,
        Err(_) => true,
    }
}

fn should_descend_org_artifact_dir(name: &str) -> bool {
    !matches!(name, ".git" | "archive" | "archives")
}

fn collect_active_org_files(root: &Path, archives_dir: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_active_org_files_into(root, archives_dir, &mut files);
    files
}

fn collect_active_org_files_into(root: &Path, archives_dir: &str, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if should_descend_active_org_artifact_dir(&name, archives_dir) {
                collect_active_org_files_into(&path, archives_dir, files);
            }
            continue;
        }
        if file_type.is_file() && is_org_file_path(&path) {
            files.push(path);
        }
    }
}

fn should_descend_active_org_artifact_dir(name: &str, archives_dir: &str) -> bool {
    !matches!(name, ".git" | "archive" | "archives") && name != archives_dir
}

fn is_org_file_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("org")
}

fn org_file_has_done_heading(path: &Path) -> bool {
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix('*') else {
            return false;
        };
        rest.trim_start().starts_with("DONE ")
    })
}

fn org_file_has_contract_binding(path: &Path) -> bool {
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    source
        .lines()
        .map(str::trim_start)
        .any(|line| line.starts_with(":CONTRACT_ORG:") || line.starts_with("#+CONTRACT_ORG:"))
}

fn artifact_display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn capture_contract_command(artifacts_root: &Path, session_id: Option<&str>) -> String {
    let plan_path = artifacts_root
        .join("flow")
        .join("plans")
        .join(agent_plan_filename(session_id));
    format!(
        "asp org capture --contract agent.plan.v1 --title 'Agent session plan' --target-file {} --no-confirm",
        shell_arg(&plan_path.display().to_string())
    )
}

fn agent_plan_filename(session_id: Option<&str>) -> String {
    let session = session_id
        .map(slugify_filename_component)
        .filter(|slug| !slug.is_empty())
        .unwrap_or_else(|| "session-unknown".to_string());
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("agent-plan-{session}-{timestamp}.org")
}

fn slugify_filename_component(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
