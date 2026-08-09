use std::path::Path;

use agent_semantic_runtime::state_core::{
    ProjectRegistryGcOptions, ProjectRegistryGcReport, ResolvedState,
    TemporaryWorkspaceCacheGcOptions, TemporaryWorkspaceCacheGcReport,
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

pub(crate) fn run_project_registry_clean(
    project_root: &Path,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let Some(args) = parse_project_registry_clean_args(forwarded_args)? else {
        return Ok(());
    };
    let options = TemporaryWorkspaceCacheGcOptions {
        apply: true,
        grace_period_ms: args.day.saturating_mul(24 * 60 * 60 * 1_000),
    };
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let state = ResolvedState::resolve_with_state_home(project_root, state_home)?;
    let report = state.gc_temporary_workspace_cache(options)?;
    print_temporary_workspace_report(&report);
    if receipt_json {
        let receipt = serde_json::to_string(&report)
            .map_err(|error| format!("failed to serialize workspace cleanup receipt: {error}"))?;
        eprintln!("{receipt}");
    }
    Ok(())
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
            "|workspace repoId={} workspaceId={} root={} protected={} eligible={} cachePresent={} retired={} ageMs={}",
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
            "|candidate repoId={} reason={} protected={} eligible={} removed={} ageMs={} roots={}",
            candidate.repo_id().as_str(),
            candidate.reason().as_str(),
            candidate.protected(),
            candidate.eligible(),
            candidate.removed(),
            candidate
                .age_ms()
                .map_or_else(|| "-".to_string(), |age| age.to_string()),
            candidate
                .recorded_checkout_roots()
                .iter()
                .map(|root| root.display().to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    if !report.apply && report.eligible_count > 0 {
        println!("next=asp cache projects gc --apply");
    }
}
