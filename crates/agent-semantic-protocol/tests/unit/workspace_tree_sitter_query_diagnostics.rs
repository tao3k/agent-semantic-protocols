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

#[test]
fn zero_match_tree_sitter_query_explains_structural_semantics() {
    let workspace = std::env::temp_dir().join(format!(
        "asp-tree-sitter-query-diagnostics-{}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).expect("create query workspace");
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
    let git_init = Command::new("git")
        .current_dir(&workspace)
        .args(["init", "--quiet"])
        .status()
        .expect("initialize Git fixture");
    assert!(git_init.success(), "Git fixture initialization failed");
    let git_add = Command::new("git")
        .current_dir(&workspace)
        .args(["add", "Cargo.toml", "lib.rs", "other.rs"])
        .status()
        .expect("index Git fixture candidates");
    assert!(git_add.success(), "Git fixture index update failed");
    let state_home = workspace.join("home/.agent-semantic-protocols");
    crate::provider_command::support::write_activation(
        &workspace,
        &[crate::provider_command::support::provider(
            "rust",
            Vec::new(),
        )],
    );
    write_rust_owner_delegate(&workspace, &state_home);
    write_provider_install_receipts(&state_home);
    let mut resident = start_fixture_resident(&workspace, &state_home);

    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.env_clear();
    for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    command.env("ASP_STATE_HOME", &state_home);
    let output = command
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
        .expect("run structural query");
    resident.kill().expect("stop fixture workspace resident");
    resident
        .wait()
        .expect("wait for fixture workspace resident");
    fs::remove_dir_all(&workspace).expect("remove query workspace");

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

#[test]
fn language_tree_sitter_search_uses_explicit_rust_facade() {
    let workspace = std::env::temp_dir().join(format!(
        "asp-tree-sitter-search-language-inference-{}",
        std::process::id()
    ));
    if workspace.exists() {
        fs::remove_dir_all(&workspace).expect("clear previous search workspace");
    }
    let state_home = workspace.join("home/.agent-semantic-protocols");
    if state_home.exists() {
        fs::remove_dir_all(&state_home).expect("clear previous search state");
    }
    fs::create_dir_all(&workspace).expect("create search workspace");
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
    let git_init = Command::new("git")
        .current_dir(&workspace)
        .args(["init", "--quiet"])
        .status()
        .expect("initialize Git fixture");
    assert!(git_init.success(), "Git fixture initialization failed");
    let git_add = Command::new("git")
        .current_dir(&workspace)
        .args(["add", "Cargo.toml", "lib.rs", "other.rs"])
        .status()
        .expect("index Git fixture candidates");
    assert!(git_add.success(), "Git fixture index update failed");
    crate::provider_command::support::write_activation(
        &workspace,
        &[
            crate::provider_command::support::provider("rust", Vec::new()),
            crate::provider_command::support::provider("typescript", Vec::new()),
            crate::provider_command::support::provider("julia", Vec::new()),
            crate::provider_command::support::provider("gerbil-scheme", Vec::new()),
        ],
    );
    write_rust_owner_delegate(&workspace, &state_home);
    write_provider_install_receipts(&state_home);
    let mut resident = start_fixture_resident(&workspace, &state_home);
    let run_search = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
        command.env_clear();
        for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
            if let Some(value) = std::env::var_os(variable) {
                command.env(variable, value);
            }
        }
        command.env("ASP_STATE_HOME", &state_home);
        command.env("ASP_PROVIDER_ACTIVATION_REFRESH", "0");
        command.env("ASP_TREESITTER_TRACE", "1");
        command
            .current_dir(&workspace)
            .args([
                "rust",
                "search",
                "--treesitter-query",
                AGENT_SESSION_LOOKUP_QUERY,
                "--workspace",
            ])
            .arg(&workspace)
            .output()
            .expect("run root structural search")
    };
    let output = run_search();

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
    assert!(stdout.contains("scheduledOwners=1"), "stdout={stdout}");
    assert!(stdout.contains("remainingOwners=1"), "stdout={stdout}");
    assert!(stdout.contains("providerParses=1"), "stdout={stdout}");
    assert!(stdout.contains("nextCommand="), "stdout={stdout}");
    assert!(stdout.contains("nextOwnerCursor="), "stdout={stdout}");
    for phase in [
        "phase=inventory-enumerate",
        "phase=inventory-probe-batch",
        "phase=initial-query-read",
        "phase=process-owner",
        "phase=final-query-read",
    ] {
        assert!(stderr.contains(phase), "phase={phase} stderr={stderr}");
    }

    let mut completed = false;
    for _ in 0..8 {
        let advance_output = run_search();
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
    let warm_output = run_search();
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
    let phase_elapsed_ms = |phase: &str| {
        warm_stderr
            .lines()
            .find(|line| {
                line.contains("[query-treesitter-phase]")
                    && line.contains(&format!("phase={phase} "))
            })
            .and_then(|line| {
                line.split_whitespace()
                    .find_map(|field| field.strip_prefix("elapsedMs="))
            })
            .and_then(|elapsed| elapsed.parse::<f64>().ok())
            .unwrap_or_else(|| panic!("missing phase timing for {phase}: stderr={warm_stderr}"))
    };
    let inventory_elapsed_ms = phase_elapsed_ms("inventory-enumerate");
    let query_elapsed_ms = phase_elapsed_ms("total");
    eprintln!(
        "[typed-project-resolution-perf] phase=warm-query queryMs={query_elapsed_ms:.3} inventoryMs={inventory_elapsed_ms:.3} wallMs={:.3}",
        warm_elapsed.as_secs_f64() * 1_000.0
    );
    assert!(
        inventory_elapsed_ms < 100.0,
        "warm typed project-resolution inventory exceeded 100ms: inventoryMs={inventory_elapsed_ms:.3} stderr={warm_stderr}"
    );
    assert!(
        query_elapsed_ms < 500.0,
        "warm incremental query exceeded 500ms: queryMs={query_elapsed_ms:.3} stderr={warm_stderr}"
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
    write_rust_owner_delegate(&workspace, &state_home);
    let dirty_output = run_search();
    resident.kill().expect("stop fixture workspace resident");
    resident
        .wait()
        .expect("wait for fixture workspace resident");
    fs::remove_dir_all(&state_home).expect("remove search state");
    fs::remove_dir_all(&workspace).expect("remove search workspace");
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

fn start_fixture_resident(
    workspace: &std::path::Path,
    state_home: &std::path::Path,
) -> std::process::Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.env_clear();
    for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    let mut child = command
        .env("ASP_STATE_HOME", state_home)
        .current_dir(workspace)
        .args(["workspace-db", "resident", "serve", "--workspace"])
        .arg(workspace)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start fixture workspace resident");
    let stdout = child.stdout.take().expect("fixture resident stdout");
    let mut reader = std::io::BufReader::new(stdout);
    let mut readiness = String::new();
    std::io::BufRead::read_line(&mut reader, &mut readiness)
        .expect("read fixture resident readiness");
    assert!(
        readiness.starts_with("[workspace-resident-service-ready]"),
        "unexpected fixture resident receipt: {readiness}"
    );
    child
}

fn write_provider_install_receipts(state_home: &std::path::Path) {
    let receipt_dir = state_home.join("runtime/providers/receipts");
    std::fs::create_dir_all(&receipt_dir).expect("create provider install receipt directory");
    for (language_id, provider_id, binary) in [
        ("rust", "rs-harness", "rs-harness"),
        ("typescript", "ts-harness", "ts-harness"),
        ("julia", "asp-julia-harness", "asp-julia-harness"),
        ("gerbil-scheme", "gslph", "gslph"),
    ] {
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
                "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"{provider_id}\"\ninstalledPath = \"{installed_path}\"\ninstalledEntrypointDigest = \"{installed_entrypoint_digest}\"\ninstalledEntrypointMetadataDigest = \"{installed_entrypoint_metadata_digest}\"\n"
            ),
        )
        .expect("write provider fixture install receipt");
    }
}

fn write_rust_owner_delegate(workspace: &std::path::Path, state_home: &std::path::Path) {
    fn digest(path: &std::path::Path) -> String {
        agent_semantic_content_identity::file_content_digest_v1(path)
            .expect("digest provider owner fixture")
    }

    fn response(
        owner_path: &str,
        source_content_digest: String,
        projections: serde_json::Value,
    ) -> String {
        serde_json::to_string(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.provider-native-owner-search-response",
            "schemaVersion": "1",
            "languageId": "rust",
            "providerId": "rs-harness",
            "requestedOwnerPath": owner_path,
            "requestedProjectionMode": "complete-owner",
            "sourceContentDigest": source_content_digest,
            "parsedOwnerCount": 1,
            "projectionCompleteness": "complete-owner",
            "projections": projections,
        }))
        .expect("encode provider owner response")
    }

    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    let lib_source =
        std::fs::read_to_string(workspace.join("lib.rs")).expect("read Rust struct owner fixture");
    let mut lib_projections = vec![serde_json::json!({
        "structuralSelector": "rust://lib.rs#item/struct/AgentSessionLookupRequest",
        "signature": "pub struct AgentSessionLookupRequest;",
        "itemKind": "struct",
        "itemName": "AgentSessionLookupRequest",
        "captureName": "declaration.name",
        "sourceByteStart": 0,
        "sourceByteEnd": 37,
    })];
    if lib_source.contains("pub struct DirtyOwner;") {
        lib_projections.push(serde_json::json!({
            "structuralSelector": "rust://lib.rs#item/struct/DirtyOwner",
            "signature": "pub struct DirtyOwner;",
            "itemKind": "struct",
            "itemName": "DirtyOwner",
            "captureName": "declaration.name",
            "sourceByteStart": 38,
            "sourceByteEnd": 60,
        }));
    }
    let lib_response = response(
        "lib.rs",
        digest(&workspace.join("lib.rs")),
        serde_json::Value::Array(lib_projections),
    );
    let other_response = response(
        "other.rs",
        digest(&workspace.join("other.rs")),
        serde_json::json!([{
            "structuralSelector": "rust://other.rs#item/function/unrelated",
            "signature": "pub fn unrelated() {}",
            "itemKind": "function",
            "itemName": "unrelated",
            "captureName": "declaration.name",
            "sourceByteStart": 0,
            "sourceByteEnd": 21,
        }]),
    );
    let project_response = serde_json::to_string(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "state": "resolved",
        "resolution": {
            "schemaId": "agent.semantic-protocols.project-resolution",
            "schemaVersion": "1",
            "state": "resolved",
            "completeness": "exact",
            "repositoryCandidates": {
                "candidates": [
                    {"path": "lib.rs"},
                    {"path": "other.rs"}
                ],
                "policyExclusions": []
            },
            "resolvedSourceScopes": [{
                "roots": ["."],
                "explicitPaths": [],
                "extensions": [".rs"],
                "includeAuthority": "package-manager",
                "exclusions": []
            }]
        }
    }))
    .expect("encode provider project-resolution response");
    let delegate = state_home.join("runtime/bin/.rs-harness-delegate");
    std::fs::write(
        &delegate,
        format!(
            "#!/bin/sh\nrequest=$(cat)\ncase \"$request\" in\n  *'\"schemaId\":\"agent.semantic-protocols.provider-project-resolution-request\"'*) printf '%s\\n' {} ;;\n  *'\"ownerPath\":\"lib.rs\"'*) printf '%s\\n' {} ;;\n  *'\"ownerPath\":\"other.rs\"'*) printf '%s\\n' {} ;;\n  *) printf '%s\\n' 'unknown typed provider fixture request' >&2; exit 64 ;;\nesac\n",
            shell_quote(&project_response),
            shell_quote(&lib_response),
            shell_quote(&other_response),
        ),
    )
    .expect("write self-contained Rust owner delegate");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&delegate)
            .expect("read Rust owner delegate metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&delegate, permissions)
            .expect("mark Rust owner delegate executable");
    }
}
