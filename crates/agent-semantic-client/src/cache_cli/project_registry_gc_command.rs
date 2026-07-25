use std::path::Path;

use agent_semantic_runtime::state_core::{
    ProjectRegistryGcOptions, ProjectRegistryGcReport, ResolvedState,
};

use super::project_registry_gc_args::parse_project_registry_gc_args;

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
