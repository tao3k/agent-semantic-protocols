use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
};

const INVALID_OWNER_PREFLIGHT_MAX_MS: u128 = 100;
const INVALID_OWNER_PREFLIGHT_TOTAL_MAX_MS: u128 = 250;

pub(in super::super) fn asp_search_owner_items_invalid_owner_preflight_stays_millisecond_scale() {
    let root = temp_project_root("scenario-search-preflight-invalid-owner");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "gslph", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let warmup = run_invalid_owner_preflight(&root, &bin_dir, "gerbil-scheme");
    assert_invalid_owner_preflight("gerbil-scheme", &warmup);

    let started_at = Instant::now();
    for language_id in ["gerbil-scheme", "rust", "typescript"] {
        let run = run_invalid_owner_preflight(&root, &bin_dir, language_id);
        assert_invalid_owner_preflight(language_id, &run);
        assert!(
            run.observed_ms <= INVALID_OWNER_PREFLIGHT_MAX_MS,
            "{language_id} invalid-owner preflight exceeded max={} observed={}ms stderr={stderr}",
            INVALID_OWNER_PREFLIGHT_MAX_MS,
            run.observed_ms,
            stderr = run.stderr
        );
    }

    assert!(
        !marker.exists(),
        "invalid-owner preflight must not invoke provider binary"
    );
    let total_ms = started_at.elapsed().as_millis();
    assert!(
        total_ms <= INVALID_OWNER_PREFLIGHT_TOTAL_MAX_MS,
        "invalid-owner scenario exceeded total max={} observed={}ms",
        INVALID_OWNER_PREFLIGHT_TOTAL_MAX_MS,
        total_ms
    );
    let _ = fs::remove_dir_all(root);
}

struct InvalidOwnerPreflightRun {
    status_code: Option<i32>,
    stderr: String,
    observed_ms: u128,
}

fn run_invalid_owner_preflight(
    root: &Path,
    bin_dir: &Path,
    language_id: &str,
) -> InvalidOwnerPreflightRun {
    let command_started_at = Instant::now();
    let output = asp_command(root)
        .env("PATH", prepend_path(bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            language_id,
            "search",
            "owner",
            ".",
            "items",
            "--query",
            "typed block Boundary",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp search owner items invalid-owner preflight scenario");
    InvalidOwnerPreflightRun {
        status_code: output.status.code(),
        stderr: String::from_utf8(output.stderr).expect("stderr"),
        observed_ms: command_started_at.elapsed().as_millis(),
    }
}

fn assert_invalid_owner_preflight(language_id: &str, run: &InvalidOwnerPreflightRun) {
    assert_eq!(
        run.status_code,
        Some(2),
        "{language_id} invalid owner must fail as a search preflight error"
    );
    assert!(
        run.stderr.contains(
            "[asp-search-query-error] code=invalid-owner owner=\".\" reason=workspace-root-owner"
        ),
        "{language_id} stderr={}",
        run.stderr
    );
    assert!(
        run.stderr.contains(&format!(
            "nextCommand=asp {language_id} search pipe '<focused terms>'"
        )),
        "{language_id} stderr={}",
        run.stderr
    );
}

fn write_marker_provider(bin_dir: &Path, binary_name: &str, marker: &Path) {
    fs::create_dir_all(bin_dir).expect("create fake provider bin dir");
    let binary = bin_dir.join(binary_name);
    fs::write(
        &binary,
        format!("#!/bin/sh\nprintf invoked > {}\n", marker.display()),
    )
    .expect("write fake provider binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&binary)
            .expect("fake provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).expect("chmod fake provider binary");
    }
}

pub(crate) fn search_owner_items_invalid_owner_preflight_scenario_gate() {
    asp_search_owner_items_invalid_owner_preflight_stays_millisecond_scale();
}
