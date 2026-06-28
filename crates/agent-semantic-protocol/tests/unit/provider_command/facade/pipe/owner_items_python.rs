use crate::provider_command::support::{
    asp_command, home_local_bin, make_executable, provider_with_owner_items, temp_project_root,
    write_activation,
};
use std::time::{Duration, Instant};

const OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE: Duration = Duration::from_millis(750);

#[test]
fn python_owner_items_hits_view_uses_provider_owned_fast_path() {
    let root = temp_project_root("search-owner-python-items-inline-fast-path");
    let bin_dir = home_local_bin(&root);
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/pkg")).expect("create source");
    std::fs::write(
        root.join("src/pkg/service.py"),
        "def fetch():\n    return 1\n\n\ndef build():\n    return 2\n",
    )
    .expect("write source");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let provider_path = bin_dir.join("py-harness");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\ncat <<'OUT'\n[search-owner] q=src/pkg/service.py pkg=. selector=items alg=provider-owned-python-owner-items\n|item fetch kind=function structuralSelector=python://src/pkg/service.py#item/function/fetch displayLineRange=1:2 sourceLocatorHint=src/pkg/service.py:1:2 reason=owner-item-skeleton-ready\nactionFrontier=A1.item-skeleton,A2.syntax-outline,A3.query-code\nrecommendedNext=A1.item-skeleton\nreason=owner-item-skeleton-ready\nOUT\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider_with_owner_items("python", Vec::new())]);

    let args = [
        "python",
        "search",
        "owner",
        "src/pkg/service.py",
        "items",
        "--query",
        "fetch",
        "--workspace",
        ".",
        "--view",
        "hits",
    ];
    let warmup = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(args)
        .output()
        .expect("warm asp python search owner items");
    assert!(
        warmup.status.success(),
        "warm stderr: {}",
        String::from_utf8_lossy(&warmup.stderr)
    );

    let mut fastest = None;
    for _ in 0..5 {
        let started_at = Instant::now();
        let output = asp_command(&root)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args(args)
            .output()
            .expect("run asp python search owner items");
        let elapsed = started_at.elapsed();
        if fastest
            .as_ref()
            .is_none_or(|(best_elapsed, _)| elapsed < *best_elapsed)
        {
            fastest = Some((elapsed, output));
        }
        if elapsed < OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE {
            break;
        }
    }
    let (elapsed, output) = fastest.expect("python owner-items sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        marker.exists(),
        "python owner-items should call provider-owned owner-items"
    );
    assert!(
        stdout.contains("alg=provider-owned-python-owner-items")
            && stdout.contains("|item fetch kind=function")
            && stdout
                .contains("structuralSelector=python://src/pkg/service.py#item/function/fetch")
            && stdout.contains("displayLineRange=1:2")
            && stdout.contains("sourceLocatorHint=src/pkg/service.py:1:2")
            && stdout.contains("actionFrontier=A1.item-skeleton,A2.syntax-outline,A3.query-code")
            && stdout.contains("recommendedNext=A1.item-skeleton")
            && stdout.contains("reason=owner-item-skeleton-ready"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("read=src/pkg/service.py:1:2"),
        "line range must not remain an executable read selector: {stdout}"
    );
    assert!(
        !stdout.contains("fallback="),
        "owner-items success output must not advertise fallback: {stdout}"
    );
    assert!(
        elapsed < OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE,
        "python owner-items debug subprocess exceeded {OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE:?}; elapsed={elapsed:?}; stdout={stdout}; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn python_owner_items_missing_owner_errors_without_fallback() {
    let root = temp_project_root("search-owner-python-items-no-fallback");
    let bin_dir = home_local_bin(&root);
    let marker = root.join("provider-called");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let provider_path = bin_dir.join("py-harness");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\nprintf 'provider should not run\\n' >&2\nexit 2\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider_with_owner_items("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "owner",
            "src/pkg/missing.py",
            "items",
            "--query",
            "fetch",
            "--workspace",
            ".",
            "--view",
            "hits",
        ])
        .output()
        .expect("run asp python search owner items");

    assert!(!output.status.success(), "owner-items should hard-fail");
    assert!(
        !marker.exists(),
        "python owner-items should not call provider after inline miss"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires an existing .py owner path")
            && stderr.contains("no fallback executed"),
        "{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("[search-owner]"),
        "missing exact owner should not render fallback owner-query: {stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
