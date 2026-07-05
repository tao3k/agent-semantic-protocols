//! Thin state-command dispatch for the `asp` binary.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::state_core::{ResolvedState, resolve_state_home};
use agent_semantic_client_db::ClientDbEngine;

/// Run the `asp` binary with pre-dispatch for State Core commands.
pub fn run_binary_from_env() -> Result<(), String> {
    if let Some(result) = run_state_command_from_env() {
        return result;
    }
    crate::run_cli_from_env()
}

fn run_state_command_from_env() -> Option<Result<(), String>> {
    let mut args = std::env::args_os();
    let _program = args.next();
    let command = args.next()?;
    if command != "state" {
        return None;
    }

    Some(run_state_command(args.collect()))
}

fn run_state_command(args: Vec<OsString>) -> Result<(), String> {
    let Some(subcommand) = args.first() else {
        return Err(state_usage());
    };
    if subcommand == "projects" {
        return run_state_projects_command(args.into_iter().skip(1).collect());
    }
    if subcommand != "locate" {
        return Err(state_usage());
    }

    let mut json = false;
    for arg in args.iter().skip(1) {
        match arg.to_str() {
            Some("--json") => json = true,
            Some("--help") | Some("-h") => {
                println!("{}", state_usage());
                return Ok(());
            }
            Some(other) => return Err(format!("unknown asp state locate option: {other}")),
            None => return Err("asp state locate received non-utf8 option".to_string()),
        }
    }

    let cwd = std::env::current_dir().map_err(|error| format!("resolve cwd: {error}"))?;
    let state = ResolvedState::resolve(&cwd)?;
    state.ensure_minimal_layout()?;
    let engine = ClientDbEngine::from_resolved_state(&state);
    engine.write_manifest()?;
    let engine_report = engine.inspect();
    let mut report = state.locate_report();
    report.db_path = engine_report.db_path;
    report.backend = engine_report.backend.to_string();
    if json {
        let body = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("serialize state locate report: {error}"))?;
        println!("{body}");
    } else {
        println!("stateHome: {}", report.state_home.display());
        println!("repoId: {}", report.repo_id);
        println!("workspaceId: {}", report.workspace_id);
        println!("scopeId: {}", report.scope_id);
        println!("repoDisplayName: {}", report.repo_display_name);
        println!("workspaceDisplayName: {}", report.workspace_display_name);
        println!("checkoutRoot: {}", report.checkout_root.display());
        if let Some(git_toplevel) = &report.git_toplevel {
            println!("gitToplevel: {}", git_toplevel.display());
        }
        if let Some(git_dir) = &report.git_dir {
            println!("gitDir: {}", git_dir.display());
        }
        if let Some(remote_url) = &report.remote_url {
            println!("remoteUrl: {remote_url}");
        }
        println!("dbPath: {}", report.db_path.display());
        println!("artifactPath: {}", report.artifact_path.display());
        println!("manifestPath: {}", report.manifest_path.display());
        println!("backend: {}", report.backend);
        println!(
            "generationManifestPath: {}",
            report.generation_manifest_path.display()
        );
        match &report.project_local_cache {
            Some(cache) => println!(
                "projectLocalCache: {} exists={}",
                cache.path.display(),
                cache.exists
            ),
            None => println!("projectLocalCache: none"),
        }
    }

    Ok(())
}

fn run_state_projects_command(args: Vec<OsString>) -> Result<(), String> {
    let Some(subcommand) = args.first() else {
        return Err(state_usage());
    };

    match subcommand.to_str() {
        Some("audit") => {
            let options =
                parse_state_projects_options("asp state projects audit", &args[1..], false)?;
            if options.help {
                return Ok(());
            }
            run_state_projects_audit_command(options)
        }
        Some("gc") | Some("prune") => {
            let options = parse_state_projects_options("asp state projects gc", &args[1..], true)?;
            if options.help {
                return Ok(());
            }
            run_state_projects_gc_command(options)
        }
        Some("--help") | Some("-h") => {
            println!("{}", state_usage());
            Ok(())
        }
        Some(other) => Err(format!("unknown asp state projects subcommand: {other}")),
        None => Err("asp state projects received non-utf8 subcommand".to_string()),
    }
}

#[derive(Default)]
struct StateProjectsCommandOptions {
    json: bool,
    apply: bool,
    help: bool,
}

fn parse_state_projects_options(
    command_name: &str,
    args: &[OsString],
    allow_apply: bool,
) -> Result<StateProjectsCommandOptions, String> {
    let mut options = StateProjectsCommandOptions::default();
    for arg in args {
        match arg.to_str() {
            Some("--json") => options.json = true,
            Some("--apply") if allow_apply => options.apply = true,
            Some("--apply") => return Err(format!("{command_name} does not accept --apply")),
            Some("--help") | Some("-h") => {
                println!("{}", state_usage());
                options.help = true;
                return Ok(options);
            }
            Some(other) => return Err(format!("unknown {command_name} option: {other}")),
            None => return Err(format!("{command_name} received non-utf8 option")),
        }
    }
    Ok(options)
}

fn run_state_projects_audit_command(options: StateProjectsCommandOptions) -> Result<(), String> {
    let state_home = resolve_state_home()?;
    let report = audit_state_projects(&state_home)?;
    if options.json {
        let body = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("serialize state projects audit report: {error}"))?;
        println!("{body}");
    } else {
        println!(
            "[state-projects-audit] owner=rust stateHome=\"{}\" total={} activeCandidates={} activeUniqueRemote={} presentCandidates={} orphanCandidates={} missingProjectJson={} tempCheckout={} noRemote={} missingCheckout={}",
            state_home.display(),
            report["summary"]["total"].as_u64().unwrap_or(0),
            report["summary"]["activeCandidate"].as_u64().unwrap_or(0),
            report["summary"]["activeUniqueRemote"]
                .as_u64()
                .unwrap_or(0),
            report["summary"]["presentCandidate"].as_u64().unwrap_or(0),
            report["summary"]["orphanCandidate"].as_u64().unwrap_or(0),
            report["summary"]["missingProjectJson"]
                .as_u64()
                .unwrap_or(0),
            report["summary"]["tempCheckout"].as_u64().unwrap_or(0),
            report["summary"]["noRemote"].as_u64().unwrap_or(0),
            report["summary"]["missingCheckout"].as_u64().unwrap_or(0),
        );
        println!("hint: rerun with `asp state projects audit --json` for per-project reasons");
    }
    Ok(())
}

fn run_state_projects_gc_command(options: StateProjectsCommandOptions) -> Result<(), String> {
    let state_home = resolve_state_home()?;
    let report = audit_state_projects(&state_home)?;
    let gc_report = state_projects_gc_report(&state_home, &report, options.apply);
    if options.json {
        let body = serde_json::to_string_pretty(&gc_report)
            .map_err(|error| format!("serialize state projects gc report: {error}"))?;
        println!("{body}");
    } else {
        println!(
            "[state-projects-gc] owner=rust stateHome=\"{}\" mode={} deleteCandidates={} deleteBlocked={} deleted={} errors={} activeCandidates={} presentCandidates={}",
            state_home.display(),
            gc_report["mode"].as_str().unwrap_or("dry-run"),
            gc_report["summary"]["deleteCandidates"]
                .as_u64()
                .unwrap_or(0),
            gc_report["summary"]["deleteBlocked"].as_u64().unwrap_or(0),
            gc_report["summary"]["deleted"].as_u64().unwrap_or(0),
            gc_report["summary"]["errors"].as_u64().unwrap_or(0),
            gc_report["summary"]["activeCandidate"]
                .as_u64()
                .unwrap_or(0),
            gc_report["summary"]["presentCandidate"]
                .as_u64()
                .unwrap_or(0),
        );
        if !options.apply {
            println!(
                "hint: rerun with `asp state projects gc --apply` to remove orphan project state directories"
            );
        }
    }
    Ok(())
}

fn audit_state_projects(state_home: &Path) -> Result<serde_json::Value, String> {
    let projects_dir = state_home.join("projects").join("by-id");
    let mut entries = Vec::new();
    if projects_dir.is_dir() {
        for entry in fs::read_dir(&projects_dir).map_err(|error| {
            format!(
                "read ASP project state directory `{}`: {error}",
                projects_dir.display()
            )
        })? {
            let entry = entry.map_err(|error| {
                format!(
                    "read ASP project state directory entry `{}`: {error}",
                    projects_dir.display()
                )
            })?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(repo_id) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !repo_id.starts_with("repo-") {
                continue;
            }
            entries.push(audit_state_project_dir(repo_id, &path, state_home));
        }
    }
    entries.sort_by(|left, right| {
        left["repoId"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["repoId"].as_str().unwrap_or_default())
    });
    classify_state_project_entries(&mut entries);

    let summary = state_project_audit_summary(&entries);
    Ok(serde_json::json!({
        "owner": "rust",
        "stateHome": state_home,
        "projectsDir": projects_dir,
        "summary": summary,
        "projects": entries,
        "deleteMode": "dry-run-only"
    }))
}

fn audit_state_project_dir(
    repo_id: &str,
    project_dir: &Path,
    state_home: &Path,
) -> serde_json::Value {
    let project_json_path = project_dir.join("project.json");
    let mut reasons = Vec::<String>::new();
    let mut classification = Vec::<String>::new();
    let mut active_blockers = Vec::<String>::new();
    let mut checkout_root = None::<String>;
    let mut remote_url = None::<String>;
    let mut identity_basis = None::<String>;
    let mut display_name = None::<String>;
    let mut project_json_valid = false;

    if !project_json_path.is_file() {
        reasons.push("missing_project_json".to_string());
    } else {
        match fs::read_to_string(&project_json_path)
            .map_err(|error| error.to_string())
            .and_then(|body| {
                serde_json::from_str::<serde_json::Value>(&body).map_err(|error| error.to_string())
            }) {
            Ok(project) => {
                project_json_valid = true;
                checkout_root = json_string_field(&project, "checkoutRoot");
                remote_url = json_string_field(&project, "remoteUrl");
                identity_basis = json_string_field(&project, "identityBasis");
                display_name = json_string_field(&project, "displayName");
            }
            Err(error) => {
                reasons.push("invalid_project_json".to_string());
                reasons.push(format!("invalid_project_json_error:{error}"));
            }
        }
    }

    if let Some(checkout_root) = checkout_root.as_deref() {
        let checkout_path = PathBuf::from(checkout_root);
        if is_temp_checkout_path(&checkout_path) {
            reasons.push("temp_checkout".to_string());
        }
        if path_is_inside_or_same(&checkout_path, state_home) {
            classification.push("state_home_checkout".to_string());
            active_blockers.push("state_home_checkout".to_string());
        }
        if path_contains_component(&checkout_path, ".data") {
            classification.push("research_checkout".to_string());
            active_blockers.push("research_checkout".to_string());
        }
        if path_contains_component(&checkout_path, "languages")
            || path_contains_component(&checkout_path, "analyzers")
        {
            classification.push("nested_checkout".to_string());
            active_blockers.push("nested_checkout".to_string());
        }
        if path_contains_component(&checkout_path, ".gerbil") {
            classification.push("dependency_checkout".to_string());
            active_blockers.push("dependency_checkout".to_string());
        }
        if !checkout_path.is_dir() {
            reasons.push("missing_checkout".to_string());
        }
    } else if project_json_valid {
        reasons.push("missing_checkout_root".to_string());
    }

    if project_json_valid && remote_url.as_deref().is_none_or(str::is_empty) {
        reasons.push("no_remote".to_string());
    }

    let workspace_count = project_dir
        .join("workspaces")
        .read_dir()
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .count()
        })
        .unwrap_or(0);
    let delete_blockers = Vec::<String>::new();
    let remote_key = remote_url.as_deref().map(normalize_remote_url);
    let stale_runtime_dirs = stale_state_project_runtime_dirs(project_dir);
    let status = if reasons.is_empty() {
        "present_candidate"
    } else {
        "orphan_candidate"
    };

    serde_json::json!({
        "repoId": repo_id,
        "projectDir": project_dir,
        "projectJson": project_json_path,
        "projectJsonValid": project_json_valid,
        "status": status,
        "activeCandidate": false,
        "dryRunDeleteCandidate": status == "orphan_candidate" && delete_blockers.is_empty(),
        "reasons": reasons,
        "classification": classification,
        "activeBlockers": active_blockers,
        "deleteBlockers": delete_blockers,
        "displayName": display_name,
        "checkoutRoot": checkout_root,
        "remoteUrl": remote_url,
        "remoteKey": remote_key,
        "identityBasis": identity_basis,
    "workspaceCount": workspace_count,
        "staleRuntimeDirCount": stale_runtime_dirs.len(),
        "staleRuntimeDirs": stale_runtime_dirs,
    })
}

fn state_projects_gc_report(
    state_home: &Path,
    audit_report: &serde_json::Value,
    apply: bool,
) -> serde_json::Value {
    let projects = audit_report["projects"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut actions = Vec::new();
    let mut deleted = 0_u64;
    let mut errors = 0_u64;
    let mut would_delete = 0_u64;
    let mut stale_runtime_dirs_removed = 0_u64;
    let mut stale_runtime_dirs_would_remove = 0_u64;

    for project in projects {
        let Some(repo_id) = project["repoId"].as_str() else {
            errors += 1;
            actions.push(serde_json::json!({
                "status": "error",
                "error": "missing repoId",
                "project": project,
            }));
            continue;
        };
        let project_delete_candidate = project["dryRunDeleteCandidate"].as_bool() == Some(true);
        let Some(project_dir) = state_project_gc_target_dir(state_home, repo_id) else {
            errors += 1;
            actions.push(serde_json::json!({
                "repoId": repo_id,
                "status": "error",
                "error": "unsafe repo id",
                "project": project,
            }));
            continue;
        };
        if project_delete_candidate && apply {
            match fs::remove_dir_all(&project_dir) {
                Ok(()) => {
                    deleted += 1;
                    actions.push(serde_json::json!({
                        "repoId": repo_id,
                        "status": "deleted",
                        "projectDir": project_dir,
                        "reasons": project["reasons"].clone(),
                    }));
                }
                Err(error) => {
                    errors += 1;
                    actions.push(serde_json::json!({
                        "repoId": repo_id,
                        "status": "error",
                        "projectDir": project_dir,
                        "error": error.to_string(),
                        "reasons": project["reasons"].clone(),
                    }));
                }
            }
            continue;
        }
        if project_delete_candidate {
            would_delete += 1;
            actions.push(serde_json::json!({
                "repoId": repo_id,
                "status": "would_delete",
                "projectDir": project_dir,
                "reasons": project["reasons"].clone(),
            }));
            continue;
        }

        for stale_dir in stale_state_project_runtime_dirs(&project_dir) {
            if apply {
                match fs::remove_dir_all(&stale_dir) {
                    Ok(()) => {
                        stale_runtime_dirs_removed += 1;
                        actions.push(serde_json::json!({
                            "repoId": repo_id,
                            "status": "removed_stale_runtime_dir",
                            "projectDir": project_dir,
                            "staleRuntimeDir": stale_dir,
                        }));
                    }
                    Err(error) => {
                        errors += 1;
                        actions.push(serde_json::json!({
                            "repoId": repo_id,
                            "status": "error",
                            "projectDir": project_dir,
                            "staleRuntimeDir": stale_dir,
                            "error": error.to_string(),
                        }));
                    }
                }
            } else {
                stale_runtime_dirs_would_remove += 1;
                actions.push(serde_json::json!({
                    "repoId": repo_id,
                    "status": "would_remove_stale_runtime_dir",
                    "projectDir": project_dir,
                    "staleRuntimeDir": stale_dir,
                }));
            }
        }
    }

    let audit_summary = &audit_report["summary"];
    serde_json::json!({
        "owner": "rust",
        "action": "state-projects-gc",
        "stateHome": state_home,
        "mode": if apply { "apply" } else { "dry-run" },
        "apply": apply,
        "deleteMode": if apply { "apply-project-and-stale-runtime-state" } else { "dry-run-only" },
        "summary": {
            "deleteCandidates": actions.len(),
            "wouldDelete": would_delete,
            "deleted": deleted,
            "staleRuntimeDirs": audit_summary["staleRuntimeDirs"].clone(),
            "staleRuntimeDirsWouldRemove": stale_runtime_dirs_would_remove,
            "staleRuntimeDirsRemoved": stale_runtime_dirs_removed,
            "errors": errors,
            "activeCandidate": audit_summary["activeCandidate"].clone(),
            "activeUniqueRemote": audit_summary["activeUniqueRemote"].clone(),
            "presentCandidate": audit_summary["presentCandidate"].clone(),
            "orphanCandidate": audit_summary["orphanCandidate"].clone(),
            "deleteBlocked": audit_summary["deleteBlocked"].clone(),
            "total": audit_summary["total"].clone(),
        },
        "projects": actions,
    })
}

fn state_project_gc_target_dir(state_home: &Path, repo_id: &str) -> Option<PathBuf> {
    if !repo_id.starts_with("repo-") {
        return None;
    }
    if repo_id.contains('/') || repo_id.contains('\\') || repo_id.contains("..") {
        return None;
    }
    Some(state_home.join("projects").join("by-id").join(repo_id))
}

fn classify_state_project_entries(entries: &mut [serde_json::Value]) {
    let remote_groups = state_project_remote_groups(entries);
    let checkout_groups = state_project_checkout_groups(entries);
    mark_duplicate_checkout_roots(entries, &checkout_groups);
    mark_duplicate_remotes(entries, &remote_groups);
    promote_active_state_project_candidates(entries);
    recompute_state_project_delete_candidates(entries);
}

fn recompute_state_project_delete_candidates(entries: &mut [serde_json::Value]) {
    for entry in entries {
        let active = entry["activeCandidate"].as_bool() == Some(true);
        let blocked = entry["deleteBlockers"]
            .as_array()
            .is_some_and(|values| !values.is_empty());
        let status_is_gc_candidate = matches!(
            entry["status"].as_str(),
            Some("present_candidate") | Some("orphan_candidate")
        );
        entry["dryRunDeleteCandidate"] =
            serde_json::json!(!active && !blocked && status_is_gc_candidate);
    }
}

fn state_project_remote_groups(entries: &[serde_json::Value]) -> BTreeMap<String, Vec<usize>> {
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry["status"].as_str() != Some("present_candidate") {
            continue;
        }
        if let Some(remote_key) = entry["remoteKey"].as_str() {
            groups
                .entry(remote_key.to_string())
                .or_default()
                .push(index);
        }
    }
    groups
}

fn state_project_checkout_groups(entries: &[serde_json::Value]) -> BTreeMap<String, Vec<usize>> {
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry["status"].as_str() != Some("present_candidate") {
            continue;
        }
        if let Some(checkout_root) = entry["checkoutRoot"].as_str() {
            groups
                .entry(checkout_root.to_string())
                .or_default()
                .push(index);
        }
    }
    groups
}

fn mark_duplicate_checkout_roots(
    entries: &mut [serde_json::Value],
    groups: &BTreeMap<String, Vec<usize>>,
) {
    let marks: Vec<(usize, bool)> = groups
        .values()
        .filter(|indices| indices.len() > 1)
        .flat_map(|indices| {
            let primary = indices[0];
            indices.iter().map(move |index| (*index, *index != primary))
        })
        .collect();
    for (index, secondary) in marks {
        push_json_string_array(
            &mut entries[index],
            "classification",
            "duplicate_checkout_root",
        );
        if secondary {
            push_json_string_array(
                &mut entries[index],
                "activeBlockers",
                "duplicate_checkout_root_secondary",
            );
        }
    }
}

fn mark_duplicate_remotes(
    entries: &mut [serde_json::Value],
    groups: &BTreeMap<String, Vec<usize>>,
) {
    let marks: Vec<(usize, bool)> = groups
        .values()
        .filter(|indices| indices.len() > 1)
        .flat_map(|indices| {
            let primary = indices
                .iter()
                .copied()
                .find(|index| json_string_array_is_empty(&entries[*index], "activeBlockers"))
                .unwrap_or(indices[0]);
            indices.iter().map(move |index| (*index, *index != primary))
        })
        .collect();
    for (index, secondary) in marks {
        push_json_string_array(&mut entries[index], "classification", "duplicate_remote");
        if secondary {
            push_json_string_array(
                &mut entries[index],
                "activeBlockers",
                "duplicate_remote_secondary",
            );
        }
    }
}

fn promote_active_state_project_candidates(entries: &mut [serde_json::Value]) {
    for entry in entries {
        if entry["status"].as_str() != Some("present_candidate") {
            entry["activeCandidate"] = serde_json::json!(false);
            continue;
        }
        let active = json_string_array_is_empty(entry, "activeBlockers");
        entry["activeCandidate"] = serde_json::json!(active);
        if active {
            entry["status"] = serde_json::json!("active_candidate");
        }
    }
}

fn stale_state_project_runtime_dirs(project_dir: &Path) -> Vec<PathBuf> {
    let mut runtime_dirs = Vec::new();
    let project_agent_dir = project_dir.join("live").join("client").join("agent");
    if project_agent_dir.is_dir() {
        runtime_dirs.push(project_agent_dir);
    }

    let workspaces_dir = project_dir.join("workspaces");
    if let Ok(workspaces) = fs::read_dir(&workspaces_dir) {
        for workspace in workspaces.filter_map(Result::ok) {
            let workspace_dir = workspace.path();
            if !workspace_dir.is_dir() {
                continue;
            }
            for relative_runtime_dir in
                ["runtime", "live/runtime", "live/hooks", "live/client/agent"]
            {
                let runtime_dir = workspace_dir.join(relative_runtime_dir);
                if runtime_dir.is_dir() {
                    runtime_dirs.push(runtime_dir);
                }
            }
        }
    }

    runtime_dirs.sort();
    runtime_dirs
}

fn state_project_audit_summary(entries: &[serde_json::Value]) -> serde_json::Value {
    let mut missing_project_json = 0_u64;
    let mut invalid_project_json = 0_u64;
    let mut temp_checkout = 0_u64;
    let mut no_remote = 0_u64;
    let mut missing_checkout = 0_u64;
    let mut active_candidate = 0_u64;
    let mut present_candidate = 0_u64;
    let mut orphan_candidate = 0_u64;
    let mut research_checkout = 0_u64;
    let mut nested_checkout = 0_u64;
    let mut state_home_checkout = 0_u64;
    let mut dependency_checkout = 0_u64;
    let mut duplicate_remote = 0_u64;
    let mut duplicate_checkout_root = 0_u64;
    let mut delete_blocked = 0_u64;
    let mut stale_runtime_dirs = 0_u64;
    let mut active_unique_remote = BTreeSet::<String>::new();

    for entry in entries {
        match entry["status"].as_str() {
            Some("active_candidate") => {
                active_candidate += 1;
                if let Some(remote_key) = entry["remoteKey"].as_str() {
                    active_unique_remote.insert(remote_key.to_string());
                }
            }
            Some("present_candidate") => present_candidate += 1,
            _ => orphan_candidate += 1,
        }
        let reasons = entry["reasons"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if json_array_contains(reasons, "missing_project_json") {
            missing_project_json += 1;
        }
        if json_array_contains(reasons, "invalid_project_json") {
            invalid_project_json += 1;
        }
        if json_array_contains(reasons, "temp_checkout") {
            temp_checkout += 1;
        }
        if json_array_contains(reasons, "no_remote") {
            no_remote += 1;
        }
        if json_array_contains(reasons, "missing_checkout")
            || json_array_contains(reasons, "missing_checkout_root")
        {
            missing_checkout += 1;
        }
        let classification = entry["classification"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if json_array_contains(classification, "research_checkout") {
            research_checkout += 1;
        }
        if json_array_contains(classification, "nested_checkout") {
            nested_checkout += 1;
        }
        if json_array_contains(classification, "state_home_checkout") {
            state_home_checkout += 1;
        }
        if json_array_contains(classification, "dependency_checkout") {
            dependency_checkout += 1;
        }
        if json_array_contains(classification, "duplicate_remote") {
            duplicate_remote += 1;
        }
        if json_array_contains(classification, "duplicate_checkout_root") {
            duplicate_checkout_root += 1;
        }
        if entry["deleteBlockers"]
            .as_array()
            .is_some_and(|values| !values.is_empty())
        {
            delete_blocked += 1;
        }
        stale_runtime_dirs += entry["staleRuntimeDirCount"].as_u64().unwrap_or(0);
    }

    serde_json::json!({
        "total": entries.len(),
        "activeCandidate": active_candidate,
        "activeUniqueRemote": active_unique_remote.len(),
        "presentCandidate": present_candidate,
        "orphanCandidate": orphan_candidate,
        "missingProjectJson": missing_project_json,
        "invalidProjectJson": invalid_project_json,
        "tempCheckout": temp_checkout,
        "noRemote": no_remote,
        "missingCheckout": missing_checkout,
        "researchCheckout": research_checkout,
        "nestedCheckout": nested_checkout,
        "stateHomeCheckout": state_home_checkout,
        "dependencyCheckout": dependency_checkout,
        "duplicateRemote": duplicate_remote,
        "duplicateCheckoutRoot": duplicate_checkout_root,
        "deleteBlocked": delete_blocked,
        "staleRuntimeDirs": stale_runtime_dirs,
    })
}

fn push_json_string_array(entry: &mut serde_json::Value, key: &str, item: &str) {
    let Some(values) = entry.get_mut(key).and_then(serde_json::Value::as_array_mut) else {
        return;
    };
    if !json_array_contains(values, item) {
        values.push(serde_json::json!(item));
    }
}

fn json_string_array_is_empty(entry: &serde_json::Value, key: &str) -> bool {
    entry[key].as_array().is_none_or(|values| values.is_empty())
}

fn json_array_contains(values: &[serde_json::Value], needle: &str) -> bool {
    values
        .iter()
        .any(|value| value.as_str().is_some_and(|value| value == needle))
}

fn json_string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn is_temp_checkout_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.starts_with("/private/var/folders/")
        || text.starts_with("/var/folders/")
        || text.starts_with("/tmp/")
}

fn path_is_inside_or_same(path: &Path, parent: &Path) -> bool {
    path == parent || path.starts_with(parent)
}

fn path_contains_component(path: &Path, component: &str) -> bool {
    let component = OsStr::new(component);
    path.components()
        .any(|candidate| candidate.as_os_str() == component)
}

fn normalize_remote_url(remote_url: &str) -> String {
    let mut value = remote_url.trim().trim_end_matches('/').to_string();
    if let Some(rest) = value.strip_prefix("ssh://git@") {
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            value = format!("{host}/{path}");
        } else {
            value = rest.to_string();
        }
    } else if let Some(rest) = value.strip_prefix("https://") {
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix("http://") {
        value = rest.to_string();
    }
    while value.ends_with(".git") {
        value.truncate(value.len() - 4);
    }
    value.to_ascii_lowercase()
}

fn state_usage() -> String {
    "usage: asp state locate [--json]\n       asp state projects audit [--json]\n       asp state projects gc [--json] [--apply]".to_string()
}
