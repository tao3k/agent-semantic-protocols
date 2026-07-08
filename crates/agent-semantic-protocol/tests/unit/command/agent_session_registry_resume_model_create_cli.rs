use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn missing_asp_explore_resume_reports_configured_model_create_instruction() {
    let temp = unique_temp_dir("asp-session-resume-model-create");
    let state_home = temp.join("state-home");
    let agents_dir = state_home.join("agents");
    fs::create_dir_all(&agents_dir).expect("create agents dir");
    fs::write(
        agents_dir.join("asp-explorer_codex.toml"),
        r#"name = "asp_explorer"
model = "gpt-5.4-mini"
sandbox_mode = "read-only"
"#,
    )
    .expect("write asp explorer config");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_THREAD_ID", "root-session-for-missing-asp-explore")
        .args([
            "agent",
            "session",
            "resume",
            "--name",
            "asp-explore",
            "--state-root",
        ])
        .arg(&state_home)
        .output()
        .expect("run asp session resume");

    assert!(
        output.status.success(),
        "resume failed: status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("name=\"asp-explore\""), "{stdout}");
    assert!(
        stdout.contains("requiredModel=\"gpt-5.4-mini\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "modelAlignmentAction=\"parent-create-resident-child-with-required-model-and-revalidate\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("model override gpt-5.4-mini and light/low reasoning"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextAction=\"create-resident-child-after-rollout-history-miss\""),
        "{stdout}"
    );

    fs::remove_dir_all(&temp).expect("cleanup temp dir");
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
