use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn exact_selector_code_reads_modified_source_without_stale_index() {
    let root = temp_project_root("exact-selector-freshness");
    fs::create_dir_all(root.join("src")).expect("create src");
    let owner = root.join("src/lib.rs");
    fs::write(&owner, "pub fn alpha() {\n    let value = 1;\n}\n").expect("write first source");

    let first = run_exact_selector_query(&root);
    assert!(first.contains("let value = 1;"), "{first}");

    fs::write(&owner, "pub fn alpha() {\n    let value = 2;\n}\n").expect("write second source");

    let second = run_exact_selector_query(&root);
    assert!(second.contains("let value = 2;"), "{second}");
    assert!(
        !second.contains("let value = 1;"),
        "exact selector query returned stale source after owner rewrite: {second}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_selector_changed_and_moved_owner_never_requires_sync() {
    let root = temp_project_root("exact-selector-live-owner");
    fs::create_dir_all(root.join("src")).expect("create src");
    let original_owner = root.join("src/lib.rs");
    fs::write(&original_owner, "pub fn live_owner() -> u8 { 1 }\n")
        .expect("write initial live owner");

    let (initial, initial_elapsed) =
        run_exact_selector_query_for(&root, "rust://src/lib.rs#item/function/live_owner");
    assert!(initial.contains("{ 1 }"), "{initial}");
    assert!(
        initial_elapsed < std::time::Duration::from_millis(250),
        "initial live-owner exact query exceeded 250ms: {initial_elapsed:?}"
    );

    let (item_missing, item_missing_elapsed) = run_exact_selector_query_for(
        &root,
        "rust://src/lib.rs#item/function/definitely_missing_live_owner_item",
    );
    assert!(
        item_missing.contains("state=item-missing"),
        "{item_missing}"
    );
    assert!(
        item_missing.contains("reasonKind=item-not-in-live-owner"),
        "{item_missing}"
    );
    assert!(
        !item_missing.contains("exact-selector provider failed"),
        "{item_missing}"
    );
    assert!(
        item_missing_elapsed < std::time::Duration::from_millis(250),
        "missing item exact query exceeded 250ms: {item_missing_elapsed:?}"
    );

    fs::write(&original_owner, "pub fn live_owner() -> u8 { 2 }\n").expect("rewrite live owner");
    let (changed, changed_elapsed) =
        run_exact_selector_query_for(&root, "rust://src/lib.rs#item/function/live_owner");
    assert!(changed.contains("{ 2 }"), "{changed}");
    assert!(!changed.contains("{ 1 }"), "{changed}");
    assert!(
        changed_elapsed < std::time::Duration::from_millis(100),
        "changed live-owner exact query exceeded 100ms: {changed_elapsed:?}"
    );

    let moved_owner = root.join("src/moved.rs");
    fs::rename(&original_owner, &moved_owner).expect("move live owner");
    let (missing, missing_elapsed) =
        run_exact_selector_query_for(&root, "rust://src/lib.rs#item/function/live_owner");
    assert!(missing.contains("state=owner-missing"), "{missing}");
    assert!(
        missing.contains("reasonKind=owner-not-in-workspace"),
        "{missing}"
    );
    assert!(!missing.contains("No such file or directory"), "{missing}");
    assert!(
        missing_elapsed < std::time::Duration::from_millis(100),
        "missing live-owner exact query exceeded 100ms: {missing_elapsed:?}"
    );

    let (moved, moved_elapsed) =
        run_exact_selector_query_for(&root, "rust://src/moved.rs#item/function/live_owner");
    assert!(moved.contains("{ 2 }"), "{moved}");
    assert!(
        moved_elapsed < std::time::Duration::from_millis(100),
        "moved live-owner exact query exceeded 100ms: {moved_elapsed:?}"
    );
    println!(
        "[exact-live-owner-performance] initialMicros={} itemMissingMicros={} changedMicros={} missingMicros={} movedMicros={} syncCount=0 wrapperByteAuthority=0",
        initial_elapsed.as_micros(),
        item_missing_elapsed.as_micros(),
        changed_elapsed.as_micros(),
        missing_elapsed.as_micros(),
        moved_elapsed.as_micros()
    );

    let _ = fs::remove_dir_all(root);
}

fn run_exact_selector_query(root: &Path) -> String {
    run_exact_selector_query_for(root, "rust://src/lib.rs#item/function/alpha").0
}

fn run_exact_selector_query_for(root: &Path, selector: &str) -> (String, std::time::Duration) {
    eprintln!(
        "[exact-query-process] binary={} root={}",
        env!("CARGO_BIN_EXE_asp"),
        root.display()
    );
    let started = std::time::Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("ASP_EXACT_QUERY_TRACE", "1")
        .arg("rust")
        .arg("query")
        .arg("--selector")
        .arg(selector)
        .arg("--workspace")
        .arg(root)
        .arg("--projection")
        .arg("source")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn asp query");
    let status = child.wait().expect("wait for asp query");
    let process_elapsed = started.elapsed();
    let pipe_started = std::time::Instant::now();
    let mut stdout = Vec::new();
    std::io::Read::read_to_end(
        child.stdout.as_mut().expect("asp query stdout"),
        &mut stdout,
    )
    .expect("read asp query stdout");
    let mut stderr = Vec::new();
    std::io::Read::read_to_end(
        child.stderr.as_mut().expect("asp query stderr"),
        &mut stderr,
    )
    .expect("read asp query stderr");
    let pipe_elapsed = pipe_started.elapsed();
    let stderr = String::from_utf8_lossy(&stderr);
    eprint!("{stderr}");
    eprintln!(
        "[exact-query-process] processMicros={} pipeDrainMicros={}",
        process_elapsed.as_micros(),
        pipe_elapsed.as_micros()
    );
    assert!(status.success(), "stderr={}", stderr);
    (
        String::from_utf8(stdout).expect("query stdout"),
        started.elapsed(),
    )
}

fn temp_project_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-{name}-{nonce}"))
}
