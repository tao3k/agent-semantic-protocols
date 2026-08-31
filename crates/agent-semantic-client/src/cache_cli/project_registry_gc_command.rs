use std::{
    collections::BTreeSet,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use agent_semantic_artifacts::{
    CatalogObservation, CleanupDisposition, CleanupPlan, ProjectBinding, RetainedObject,
    RetentionLease, RetentionObjectKind, StateHomeCatalog, StateHomeLayout,
};

use agent_semantic_runtime::state_core::{
    ProjectRegistryGcCandidate, ProjectRegistryGcOptions, ProjectRegistryGcReport, ResolvedState,
    TemporaryWorkspaceCacheGcCandidate, TemporaryWorkspaceCacheGcOptions,
    TemporaryWorkspaceCacheGcReport,
};

use super::project_registry_gc_args::{
    parse_project_registry_clean_args, parse_project_registry_gc_args,
};

pub(crate) fn run_project_registry_gc(
    project_root: &Path,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let Some(args) = parse_project_registry_gc_args(forwarded_args)? else {
        return Ok(());
    };
    let options = ProjectRegistryGcOptions {
        apply: args.apply,
        grace_period_ms: args.grace_days.saturating_mul(24 * 60 * 60 * 1_000),
    };
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let state = ResolvedState::resolve_with_state_home(project_root, state_home)?;
    let report = state.gc_project_registry(options)?;
    print_report(&report);
    if receipt_json {
        let receipt = serde_json::to_string(&report)
            .map_err(|error| format!("failed to serialize project GC receipt: {error}"))?;
        eprintln!("{receipt}");
    }
    Ok(())
}

pub(crate) async fn run_project_registry_clean(
    project_root: &Path,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let Some(args) = parse_project_registry_clean_args(forwarded_args)? else {
        return Ok(());
    };
    let grace_period_ms = args.day.saturating_mul(24 * 60 * 60 * 1_000);
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let state = ResolvedState::resolve_with_state_home(project_root, state_home)?;
    let temporary_scan = state.gc_temporary_workspace_cache(TemporaryWorkspaceCacheGcOptions {
        apply: false,
        grace_period_ms,
    })?;
    let project_scan = state.gc_project_registry(ProjectRegistryGcOptions {
        apply: false,
        grace_period_ms,
    })?;
    let (catalog_generation, catalog_plan) =
        admit_cleanup_with_catalog(&state, &temporary_scan, &project_scan, grace_period_ms).await?;
    validate_catalog_admission(&temporary_scan, &project_scan, &catalog_plan)?;

    let temporary_report =
        state.gc_temporary_workspace_cache(TemporaryWorkspaceCacheGcOptions {
            apply: true,
            grace_period_ms,
        })?;
    let project_report = state.gc_project_registry(ProjectRegistryGcOptions {
        apply: true,
        grace_period_ms,
    })?;
    print_temporary_workspace_report(&temporary_report);
    print_report(&project_report);
    println!(
        "[asp-state-home-clean] status=applied retainedForDays={} catalogGeneration={} temporaryRetired={} projectsRetired={} retiredBytes={}",
        args.day,
        catalog_generation,
        temporary_report.retired_count,
        project_report.removed_count,
        temporary_report
            .candidates
            .iter()
            .filter(|candidate| candidate.retired())
            .map(TemporaryWorkspaceCacheGcCandidate::byte_count)
            .sum::<u64>()
            .saturating_add(
                project_report
                    .candidates
                    .iter()
                    .filter(|candidate| candidate.removed())
                    .map(ProjectRegistryGcCandidate::byte_count)
                    .sum::<u64>(),
            ),
    );
    if receipt_json {
        let receipt = serde_json::to_string(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.state-home-cleanup-receipt",
            "schemaVersion": 1,
            "state": "applied",
            "retainedForDays": args.day,
            "catalogGeneration": catalog_generation,
            "catalogPlan": catalog_plan,
            "temporaryWorkspaceReport": temporary_report,
            "projectRegistryReport": project_report,
        }))
        .map_err(|error| format!("failed to serialize workspace cleanup receipt: {error}"))?;
        eprintln!("{receipt}");
    }
    Ok(())
}

async fn admit_cleanup_with_catalog(
    state: &ResolvedState,
    temporary_scan: &TemporaryWorkspaceCacheGcReport,
    project_scan: &ProjectRegistryGcReport,
    grace_period_ms: u64,
) -> Result<(u64, CleanupPlan), String> {
    let evaluated_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_millis()
        .try_into()
        .map_err(|_| "system timestamp exceeds u64".to_string())?;
    let layout = StateHomeLayout::new(&state.state_home);
    let catalog = StateHomeCatalog::open(&layout.catalog).await?;
    let mut observations = Vec::with_capacity(
        temporary_scan
            .candidates
            .len()
            .saturating_add(project_scan.candidates.len()),
    );
    for candidate in &temporary_scan.candidates {
        let object_id = temporary_object_id(candidate);
        observations.push(CatalogObservation {
            binding: ProjectBinding::resolve(
                None,
                format!("legacy-repo-id:{}", candidate.repo_id().as_str()),
                candidate.root(),
            )?,
            object: RetainedObject {
                object_id: object_id.clone(),
                kind: RetentionObjectKind::Workspace,
                last_observed_at_ms: candidate.last_seen_ms().unwrap_or(evaluated_at_ms),
                byte_count: candidate.byte_count(),
            },
            leases: candidate
                .protected()
                .then(|| RetentionLease {
                    lease_id: format!("current-workspace:{object_id}"),
                    object_id,
                    owner: "state-home-resolution".to_string(),
                    expires_at_ms: None,
                })
                .into_iter()
                .collect(),
            observed_at_ms: evaluated_at_ms,
        });
    }
    for candidate in &project_scan.candidates {
        let object_id = project_object_id(candidate);
        let workspace_root = candidate
            .recorded_checkout_roots()
            .first()
            .map_or_else(|| candidate.project_dir(), |root| root.as_path());
        observations.push(CatalogObservation {
            binding: ProjectBinding::resolve(
                None,
                format!("legacy-repo-id:{}", candidate.repo_id().as_str()),
                workspace_root,
            )?,
            object: RetainedObject {
                object_id: object_id.clone(),
                kind: RetentionObjectKind::Project,
                last_observed_at_ms: candidate.last_seen_ms().unwrap_or(evaluated_at_ms),
                byte_count: candidate.byte_count(),
            },
            leases: candidate
                .protected()
                .then(|| RetentionLease {
                    lease_id: format!("current-project:{object_id}"),
                    object_id,
                    owner: "state-home-resolution".to_string(),
                    expires_at_ms: None,
                })
                .into_iter()
                .collect(),
            observed_at_ms: evaluated_at_ms,
        });
    }
    let generation = if observations.is_empty() {
        catalog.generation().await?
    } else {
        catalog.observe_batch(&observations).await?.generation
    };
    let plan = catalog
        .plan_cleanup(evaluated_at_ms, grace_period_ms)
        .await?;
    Ok((generation.get(), plan))
}

fn validate_catalog_admission(
    temporary_scan: &TemporaryWorkspaceCacheGcReport,
    project_scan: &ProjectRegistryGcReport,
    plan: &CleanupPlan,
) -> Result<(), String> {
    let retired = plan
        .entries
        .iter()
        .filter_map(|entry| {
            matches!(entry.disposition, CleanupDisposition::Retire { .. })
                .then_some(entry.object.object_id.as_str())
        })
        .collect::<BTreeSet<_>>();
    for candidate in temporary_scan
        .candidates
        .iter()
        .filter(|candidate| candidate.eligible())
    {
        let object_id = temporary_object_id(candidate);
        if !retired.contains(object_id.as_str()) {
            return Err(format!(
                "State Home catalog retained cleanup candidate {object_id}"
            ));
        }
    }
    for candidate in project_scan
        .candidates
        .iter()
        .filter(|candidate| candidate.eligible())
    {
        let object_id = project_object_id(candidate);
        if !retired.contains(object_id.as_str()) {
            return Err(format!(
                "State Home catalog retained cleanup candidate {object_id}"
            ));
        }
    }
    Ok(())
}

fn temporary_object_id(candidate: &TemporaryWorkspaceCacheGcCandidate) -> String {
    format!(
        "legacy-workspace:{}:{}",
        candidate.repo_id().as_str(),
        candidate.workspace_id().as_str()
    )
}

fn project_object_id(candidate: &ProjectRegistryGcCandidate) -> String {
    format!("legacy-project:{}", candidate.repo_id().as_str())
}

fn print_temporary_workspace_report(report: &TemporaryWorkspaceCacheGcReport) {
    println!(
        "[asp-cache-temporary-workspaces] status={} scanned={} temporary={} eligible={} retired={} graceMs={} stateHome={}",
        if report.apply { "applied" } else { "dry-run" },
        report.scanned_workspace_count,
        report.temporary_workspace_count,
        report.eligible_count,
        report.retired_count,
        report.grace_period_ms,
        report.state_home.display()
    );
    for candidate in &report.candidates {
        println!(
            "|workspace repoId={} workspaceId={} root={} protected={} eligible={} cachePresent={} retired={} ageMs={} bytes={} retentionReason={} observationAuthority=legacy-filesystem-import",
            candidate.repo_id().as_str(),
            candidate.workspace_id().as_str(),
            candidate.root().display(),
            candidate.protected(),
            candidate.eligible(),
            candidate.cache_present(),
            candidate.retired(),
            candidate
                .age_ms()
                .map_or_else(|| "-".to_string(), |age| age.to_string()),
            candidate.byte_count(),
            candidate.retention_reason(),
        );
    }
}

fn print_report(report: &ProjectRegistryGcReport) {
    println!(
        "[asp-cache-projects] status={} scanned={} candidates={} eligible={} removed={} graceMs={} stateHome={}",
        if report.apply { "applied" } else { "dry-run" },
        report.scanned_project_count,
        report.candidate_count,
        report.eligible_count,
        report.removed_count,
        report.grace_period_ms,
        report.state_home.display()
    );
    for candidate in &report.candidates {
        println!(
            "|candidate repoId={} reason={} protected={} eligible={} removed={} ageMs={} bytes={} retentionReason={} observationAuthority=legacy-filesystem-import roots={}",
            candidate.repo_id().as_str(),
            candidate.reason().as_str(),
            candidate.protected(),
            candidate.eligible(),
            candidate.removed(),
            candidate
                .age_ms()
                .map_or_else(|| "-".to_string(), |age| age.to_string()),
            candidate.byte_count(),
            candidate.retention_reason(),
            candidate
                .recorded_checkout_roots()
                .iter()
                .map(|root| root.display().to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    if !report.apply && report.eligible_count > 0 {
        println!("next=asp cache gc --apply");
    }
}
