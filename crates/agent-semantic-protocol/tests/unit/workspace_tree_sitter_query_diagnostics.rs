use std::fs;
use std::process::Command;

const IMPOSSIBLE_RUST_IDENTIFIER_QUERY: &str = r#"
(function_item
  name: (identifier) @declaration.name
  (#eq? @declaration.name "direct-source-read"))
"#;

const AGENT_SESSION_LOOKUP_QUERY: &str = r#"
((type_identifier) @reference.name
  (#eq? @reference.name "AgentSessionLookupRequest"))
"#;

#[tokio::test(flavor = "current_thread")]
async fn zero_match_tree_sitter_query_explains_structural_semantics() {
    let workspace = create_linked_fixture_workspace("asp-tree-sitter-query-diagnostics");
    fs::write(
        workspace.join("lib.rs"),
        "pub struct AgentSessionLookupRequest;\n",
    )
    .expect("write Rust fixture");
    fs::write(workspace.join("other.rs"), "pub fn unrelated() {}\n")
        .expect("write second Rust owner");
    fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"tree-sitter-zero-match-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\npath = \"lib.rs\"\n",
    )
    .expect("write Rust project entry");
    let git_add = Command::new("git")
        .current_dir(&workspace)
        .args(["add", "Cargo.toml", "lib.rs", "other.rs"])
        .status()
        .expect("index Git fixture candidates");
    assert!(git_add.success(), "Git fixture index update failed");
    let git_commit = Command::new("/usr/bin/git")
        .current_dir(&workspace)
        .env("PREK_ALLOW_NO_CONFIG", "1")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "user.name=ASP Fixture",
            "-c",
            "user.email=asp-fixture@invalid",
        ])
        .args([
            "-c",
            "user.name=ASP Fixture",
            "-c",
            "user.email=asp-fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "materialize canonical fixture workspace",
        ])
        .status()
        .expect("commit Git fixture");
    assert!(git_commit.success(), "Git fixture commit failed");
    let state_home = workspace.join("home/.agent-semantic-protocols");
    crate::provider_command::support::write_activation(
        &workspace,
        &[crate::provider_command::support::provider(
            "rust",
            Vec::new(),
        )],
    );
    let rust_provider = build_fixture_rust_provider().await;
    crate::provider_command::support::install_state_home_provider(
        &workspace,
        "rust",
        &rust_provider,
    );
    write_provider_install_receipts(&state_home);
    let server_start_at = std::time::Instant::now();
    let mut resident = start_fixture_resident(&workspace, &state_home).await;
    eprintln!(
        "[runtime-tree-sitter-perf] phase=server-start wallMs={:.3}",
        server_start_at.elapsed().as_secs_f64() * 1_000.0
    );
    assert!(
        server_start_at.elapsed() < std::time::Duration::from_secs(5),
        "fixture Runtime Server startup exceeded 5s: elapsed={:?}",
        server_start_at.elapsed()
    );
    let generation_admission_at = std::time::Instant::now();
    admit_linked_fixture_generation(&workspace, &state_home).await;
    eprintln!(
        "[runtime-tree-sitter-perf] phase=generation-admission wallMs={:.3}",
        generation_admission_at.elapsed().as_secs_f64() * 1_000.0
    );
    assert!(
        generation_admission_at.elapsed() < std::time::Duration::from_secs(5),
        "fixture generation admission exceeded 5s: elapsed={:?}",
        generation_admission_at.elapsed()
    );

    let run_search = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
        command.env_clear();
        for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
            if let Some(value) = std::env::var_os(variable) {
                command.env(variable, value);
            }
        }
        command.env("ASP_STATE_HOME", &state_home);
        command
            .current_dir(&workspace)
            .args([
                "search",
                "--language",
                "rust",
                "--treesitter-query",
                IMPOSSIBLE_RUST_IDENTIFIER_QUERY,
                "--workspace",
            ])
            .arg(&workspace)
            .output()
            .expect("run structural query")
    };
    let query_started_at = std::time::Instant::now();
    let mut output = run_search();
    assert!(
        query_started_at.elapsed() < std::time::Duration::from_secs(2),
        "initial Runtime Tree-sitter query process exceeded 2s: elapsed={:?}",
        query_started_at.elapsed()
    );
    for _ in 0..8 {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("state=complete") {
            break;
        }
        assert!(
            output.status.success(),
            "stdout={}\nstderr={}",
            stdout,
            String::from_utf8_lossy(&output.stderr)
        );
        output = run_search();
    }
    resident
        .kill()
        .await
        .expect("stop fixture workspace resident");
    remove_linked_fixture_workspace(&workspace);

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert_eq!(stdout.matches("[search-treesitter]").count(), 1);
    assert!(stdout.contains("status=no-matches"), "stdout={stdout}");
    assert!(
        stdout.contains("mode: structural Tree-sitter reasoning search"),
        "stdout={stdout}",
    );
    assert!(
        stdout.contains("use `_` rather than `-`"),
        "stdout={stdout}",
    );
    assert!(stdout.contains("asp rust search pipe"), "stdout={stdout}");
}

pub(crate) fn create_linked_fixture_workspace(name: &str) -> std::path::PathBuf {
    let workspace = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let owner_checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if workspace.exists() {
        let removed = Command::new("git")
            .current_dir(owner_checkout)
            .args(["worktree", "remove", "--force"])
            .arg(&workspace)
            .status()
            .expect("remove previous linked fixture checkout");
        assert!(removed.success(), "previous linked fixture removal failed");
    }
    let linked = Command::new("git")
        .current_dir(owner_checkout)
        .args(["worktree", "add", "--detach", "--no-checkout"])
        .arg(&workspace)
        .arg("HEAD")
        .status()
        .expect("create linked fixture checkout");
    assert!(linked.success(), "linked fixture checkout failed");
    let empty_index = Command::new("git")
        .current_dir(&workspace)
        .args(["read-tree", "--empty"])
        .status()
        .expect("empty linked fixture index");
    assert!(empty_index.success(), "empty linked fixture index failed");
    fs::write(
        workspace.join(".pre-commit-config.yaml"),
        "repos:\n  - repo: local\n    hooks:\n      - id: fixture\n        name: fixture\n        entry: /usr/bin/true\n        language: system\n        pass_filenames: false\n",
    )
    .expect("write fixture pre-commit config");
    workspace
}

fn remove_linked_fixture_workspace(workspace: &std::path::Path) {
    let removed = Command::new("git")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["worktree", "remove", "--force"])
        .arg(workspace)
        .status()
        .expect("remove linked fixture checkout");
    assert!(removed.success(), "linked fixture checkout removal failed");
}

pub(crate) fn commit_linked_fixture_workspace(workspace: &std::path::Path) {
    let committed = Command::new("/usr/bin/git")
        .current_dir(workspace)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "user.name=ASP Fixture",
            "-c",
            "user.email=asp-fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "materialize canonical fixture workspace",
        ])
        .status()
        .expect("commit linked fixture checkout");
    assert!(committed.success(), "linked fixture checkout commit failed");
}

async fn build_fixture_rust_provider() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let harness_manifest =
        manifest_dir.join("../../languages/rust-lang-project-harness/Cargo.toml");
    let harness_manifest = tokio::fs::canonicalize(&harness_manifest)
        .await
        .expect("canonicalize Rust provider harness manifest");
    let output = tokio::process::Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            harness_manifest
                .to_str()
                .expect("Rust provider harness manifest is UTF-8"),
            "--features",
            "cli",
            "--bin",
            "rs-harness",
        ])
        .output()
        .await
        .expect("build fixture Rust provider");
    assert!(
        output.status.success(),
        "fixture Rust provider build failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    harness_manifest
        .parent()
        .expect("Rust provider harness manifest parent")
        .join("target/debug/rs-harness")
}

#[tokio::test(flavor = "current_thread")]
async fn language_tree_sitter_search_uses_explicit_rust_facade() {
    let workspace = create_linked_fixture_workspace("asp-tree-sitter-search-language-inference");
    let state_home = workspace.join("home/.agent-semantic-protocols");
    if state_home.exists() {
        fs::remove_dir_all(&state_home).expect("clear previous search state");
    }
    fs::write(
        workspace.join("lib.rs"),
        "pub struct AgentSessionLookupRequest;\n",
    )
    .expect("write Rust fixture");
    fs::write(workspace.join("other.rs"), "pub fn unrelated() {}\n")
        .expect("write second Rust owner");
    fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"tree-sitter-query-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\npath = \"lib.rs\"\n",
    )
    .expect("write Rust project entry");
    let git_add = Command::new("git")
        .current_dir(&workspace)
        .args(["add", "Cargo.toml", "lib.rs", "other.rs"])
        .status()
        .expect("index Git fixture candidates");
    assert!(git_add.success(), "Git fixture index update failed");
    commit_linked_fixture_workspace(&workspace);
    let rust_provider = build_fixture_rust_provider().await;
    crate::provider_command::support::install_state_home_provider(
        &workspace,
        "rust",
        &rust_provider,
    );
    crate::provider_command::support::write_activation(
        &workspace,
        &[
            crate::provider_command::support::provider("rust", Vec::new()),
            crate::provider_command::support::provider("typescript", Vec::new()),
            crate::provider_command::support::provider("julia", Vec::new()),
            crate::provider_command::support::provider("gerbil-scheme", Vec::new()),
        ],
    );
    write_provider_install_receipts(&state_home);
    let mut resident = start_fixture_resident(&workspace, &state_home).await;
    admit_linked_fixture_generation(&workspace, &state_home).await;
    admit_linked_fixture_generation(&workspace, &state_home).await;
    let output = run_fixture_search(&workspace, &state_home).await;

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let client_db_paths = collect_files_named(&state_home, "facts.turso");
    assert_eq!(
        client_db_paths.len(),
        1,
        "expected one canonical Turso client DB below {}, got {client_db_paths:?}",
        state_home.display()
    );
    assert!(
        client_db_paths[0].starts_with(&state_home)
            && client_db_paths[0]
                .components()
                .any(|component| component.as_os_str() == "live"),
        "canonical Turso client DB path drift: {}",
        client_db_paths[0].display()
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
    let mut trace_history = vec![stderr.clone()];
    assert_eq!(stdout.matches("[search-treesitter]").count(), 1);
    assert!(stdout.contains("language=rust"), "stdout={stdout}");
    assert!(stdout.contains("state=partial"), "stdout={stdout}");
    assert!(
        stdout.contains("scheduledOwners=1"),
        "missing scheduledOwners=1 in initial partial receipt: stdout={stdout}"
    );
    assert!(
        stdout.contains("remainingOwners=1"),
        "missing remainingOwners=1 in initial partial receipt: stdout={stdout}"
    );
    assert!(
        stdout.contains("providerParses=1"),
        "missing providerParses=1 in initial partial receipt: stdout={stdout}"
    );
    assert!(
        stdout.contains("next:"),
        "missing compact continuation command in initial partial receipt: stdout={stdout}"
    );
    assert!(
        stdout.contains("nextOwnerCursor="),
        "missing nextOwnerCursor in initial partial receipt: stdout={stdout}"
    );

    let mut completed = false;
    for _ in 0..8 {
        let advance_output = run_fixture_search(&workspace, &state_home).await;
        assert!(
            advance_output.status.success(),
            "advance stdout={}\nadvance stderr={}",
            String::from_utf8_lossy(&advance_output.stdout),
            String::from_utf8_lossy(&advance_output.stderr),
        );
        let advance_stdout =
            String::from_utf8(advance_output.stdout).expect("advance utf-8 stdout");
        trace_history.push(String::from_utf8(advance_output.stderr).expect("advance utf-8 stderr"));
        assert!(
            advance_stdout.contains("reference.name"),
            "stdout={advance_stdout}"
        );
        assert!(
            advance_stdout.contains("scheduledOwners=0")
                || advance_stdout.contains("scheduledOwners=1"),
            "budget drift stdout={advance_stdout}"
        );
        assert!(
            advance_stdout.contains("providerParses=0")
                || advance_stdout.contains("providerParses=1"),
            "provider subprocess budget drift stdout={advance_stdout}"
        );
        if advance_stdout.contains("state=complete") {
            completed = true;
            break;
        }
        assert!(
            advance_stdout.contains("providerParses=1"),
            "partial call made no bounded progress stdout={advance_stdout}"
        );
    }
    assert!(
        completed,
        "incremental fixture did not complete within owner bound"
    );

    let warm_started = std::time::Instant::now();
    let warm_output = run_fixture_search(&workspace, &state_home).await;
    let warm_elapsed = warm_started.elapsed();
    eprintln!(
        "[typed-project-resolution-perf] phase=warm elapsedMs={:.3}",
        warm_elapsed.as_secs_f64() * 1_000.0
    );
    assert!(
        warm_output.status.success(),
        "warm stdout={}\nwarm stderr={}",
        String::from_utf8_lossy(&warm_output.stdout),
        String::from_utf8_lossy(&warm_output.stderr),
    );
    let warm_stdout = String::from_utf8(warm_output.stdout).expect("warm utf-8 stdout");
    let warm_stderr = String::from_utf8_lossy(&warm_output.stderr);
    eprintln!(
        "[typed-project-resolution-perf] phase=warm-query wallMs={:.3}",
        warm_elapsed.as_secs_f64() * 1_000.0
    );
    for counter in [
        "sourceReads=0",
        "providerParses=0",
        "queryWrites=0",
        "ownerWrites=0",
        "fullWalks=0",
        "casWrites=0",
        "fullMerkle=0",
        "unrelatedProviders=0",
    ] {
        assert!(
            warm_stdout.contains(counter),
            "counter={counter} stdout={warm_stdout} stderr={warm_stderr} history={trace_history:?}"
        );
    }

    fs::write(
        workspace.join("lib.rs"),
        "pub struct AgentSessionLookupRequest;\npub struct DirtyOwner;\n",
    )
    .expect("change one Rust owner");
    let dirty_output = run_fixture_search(&workspace, &state_home).await;
    resident
        .kill()
        .await
        .expect("stop fixture workspace resident");
    fs::remove_dir_all(&state_home).expect("remove search state");
    remove_linked_fixture_workspace(&workspace);
    assert!(
        dirty_output.status.success(),
        "dirty stdout={}\ndirty stderr={}",
        String::from_utf8_lossy(&dirty_output.stdout),
        String::from_utf8_lossy(&dirty_output.stderr),
    );
    let dirty_stdout = String::from_utf8(dirty_output.stdout).expect("dirty utf-8 stdout");
    for counter in [
        "sourceReads=1",
        "providerParses=1",
        "queryWrites=1",
        "ownerWrites=1",
        "fullWalks=0",
        "casWrites=0",
        "fullMerkle=0",
        "unrelatedProviders=0",
    ] {
        assert!(
            dirty_stdout.contains(counter),
            "counter={counter} stdout={dirty_stdout}"
        );
    }
}

fn collect_files_named(root: &std::path::Path, file_name: &str) -> Vec<std::path::PathBuf> {
    fn visit(directory: &std::path::Path, file_name: &str, matches: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(directory)
            .expect("read canonical state directory")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, file_name, matches);
            } else if path.file_name().and_then(std::ffi::OsStr::to_str) == Some(file_name) {
                matches.push(path);
            }
        }
    }

    let mut matches = Vec::new();
    visit(root, file_name, &mut matches);
    matches.sort();
    matches
}

struct FixtureRuntimeServer {
    child: tokio::process::Child,
}

impl FixtureRuntimeServer {
    async fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill().await?;
        self.child.wait().await?;
        Ok(())
    }
}

async fn start_fixture_resident(
    workspace: &std::path::Path,
    state_home: &std::path::Path,
) -> FixtureRuntimeServer {
    agent_semantic_protocol::prepare_runtime_server_provider_catalog(state_home)
        .expect("reconcile fixture Runtime provider catalog");
    let runtime_artifact = state_home.join("runtime/bin/asp");
    assert!(
        runtime_artifact.is_file(),
        "fixture Runtime Server artifact is unavailable: {}",
        runtime_artifact.display()
    );
    let mut command = tokio::process::Command::new(&runtime_artifact);
    command.env_clear();
    for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    let child = command
        .env("ASP_STATE_HOME", state_home)
        .env("ASP_PROVIDER_TIMEOUT_MS", "5000")
        .env("ASP_PROVIDER_TIMEOUT_MS", "5000")
        .env("ASP_PROVIDER_TIMEOUT_MS", "5000")
        .env("ASP_PROVIDER_TIMEOUT_MS", "5000")
        .current_dir(workspace)
        .args(["server", "daemon"])
        .kill_on_drop(true)
        .spawn()
        .expect("spawn fixture ASP Server daemon");
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if agent_semantic_client_db::read_runtime_server_endpoint(state_home)
            .ok()
            .flatten()
            .is_some()
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "fixture ASP Server daemon did not publish its endpoint"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    FixtureRuntimeServer { child }
}

async fn run_fixture_search(
    workspace: &std::path::Path,
    state_home: &std::path::Path,
) -> std::process::Output {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_asp"));
    command.env_clear();
    for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    command
        .env("ASP_STATE_HOME", state_home)
        .env("ASP_PROVIDER_ACTIVATION_REFRESH", "0")
        .env("ASP_TREESITTER_TRACE", "1")
        .current_dir(workspace)
        .args([
            "rust",
            "search",
            "--treesitter-query",
            AGENT_SESSION_LOOKUP_QUERY,
            "--workspace",
        ])
        .arg(workspace)
        .output()
        .await
        .expect("run root structural search")
}

async fn admit_linked_fixture_generation(
    workspace: &std::path::Path,
    state_home: &std::path::Path,
) {
    let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(state_home)
        .expect("read fixture Runtime Server endpoint")
        .expect("fixture Runtime Server endpoint is available");
    agent_semantic_client_db::runtime_server_control::ensure_runtime_server_workspace(
        &endpoint,
        workspace,
        "tree-sitter-fixture-bootstrap".to_owned(),
    )
    .await
    .expect("bootstrap linked fixture generation");
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(workspace)
            .expect("resolve linked fixture state")
            .workspace
            .workspace_id
            .to_string();
    let session = agent_semantic_client_db::WorkspaceDbIpcSession::for_runtime_server(
        &endpoint,
        workspace_identity,
        tokio::fs::canonicalize(workspace)
            .await
            .expect("canonicalize linked fixture workspace"),
    );
    let admission = session
        .admit_runtime_generation(
            "tree-sitter-fixture-owner-delta",
            vec!["lib.rs".to_owned(), "other.rs".to_owned()],
        )
        .await
        .expect("admit linked fixture owner delta");
    let receipt = admission
        .receipts
        .first()
        .expect("fixture mutation admission receipt");
    assert!(
        matches!(
            receipt.state,
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
        ) && receipt.commit.is_some()
            && receipt.error.is_none(),
        "fixture generation was not committed: {receipt:?}"
    );
    let authority = session
        .runtime_search_generation_authority()
        .await
        .expect("read fixture search generation authority");
    assert!(
        authority.workspace_generation.owner_count >= 2,
        "fixture search generation has no published Rust owners: {authority:?}"
    );
}

fn write_provider_install_receipts(state_home: &std::path::Path) {
    let receipt_dir = state_home.join("runtime/providers/receipts");
    std::fs::create_dir_all(&receipt_dir).expect("create provider install receipt directory");
    for registration in agent_semantic_hook::registered_provider_binaries_v1() {
        let language_id = registration.language_id().as_str();
        let provider_id = registration.provider_id().as_str();
        let binary = registration.binary();
        let installed_path = state_home.join("runtime/bin").join(binary);
        if !installed_path.is_file() {
            continue;
        }
        let installed_entrypoint_digest =
            agent_semantic_content_identity::file_content_digest_v1(&installed_path)
                .expect("digest provider fixture binary");
        let installed_entrypoint_metadata_digest =
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed_path)
                .expect("digest provider fixture metadata");
        let installed_path = installed_path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        std::fs::write(
            receipt_dir.join(format!("{language_id}.lock.toml")),
            format!(
                "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{language_id}\"\nprovider = \"{provider_id}\"\ninstalledPath = \"{installed_path}\"\ninstalledEntrypointDigest = \"{installed_entrypoint_digest}\"\ninstalledEntrypointMetadataDigest = \"{installed_entrypoint_metadata_digest}\"\n"
            ),
        )
        .expect("write provider fixture install receipt");
    }
}
