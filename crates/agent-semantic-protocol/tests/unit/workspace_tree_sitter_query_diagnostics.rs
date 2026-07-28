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
    fs::write(workspace.join("lib.rs"), "fn direct_source_read() {}\n")
        .expect("write Rust fixture");

    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.env_clear();
    for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
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
    let state_home = workspace.with_extension("asp-state");
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
    crate::provider_command::support::write_activation(
        &workspace,
        &[
            crate::provider_command::support::provider("rust", Vec::new()),
            crate::provider_command::support::provider("typescript", Vec::new()),
            crate::provider_command::support::provider("julia", Vec::new()),
            crate::provider_command::support::provider("gerbil-scheme", Vec::new()),
        ],
    );

    isolate_test_provider_cwd(&workspace);

    let run_search = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
        command.env_clear();
        for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
            if let Some(value) = std::env::var_os(variable) {
                command.env(variable, value);
            }
        }
        command.env("PRJ_CACHE_HOME", workspace.join(".cache"));
        command.env("AST_STATE_HOME", &state_home);
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
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
    assert_eq!(stdout.matches("[search-treesitter]").count(), 1);
    assert!(stdout.contains("language=rust"), "stdout={stdout}");
    assert!(stdout.contains("state=partial"), "stdout={stdout}");
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

    let warm_output = run_search();
    assert!(
        warm_output.status.success(),
        "warm stdout={}\nwarm stderr={}",
        String::from_utf8_lossy(&warm_output.stdout),
        String::from_utf8_lossy(&warm_output.stderr),
    );
    let warm_stdout = String::from_utf8(warm_output.stdout).expect("warm utf-8 stdout");
    let warm_stderr = String::from_utf8_lossy(&warm_output.stderr);
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
            "counter={counter} stdout={warm_stdout} stderr={warm_stderr}"
        );
    }

    fs::write(
        workspace.join("lib.rs"),
        "pub struct AgentSessionLookupRequest;\npub struct DirtyOwner;\n",
    )
    .expect("change one Rust owner");
    let dirty_output = run_search();
    fs::remove_dir_all(&workspace).expect("remove search workspace");
    fs::remove_dir_all(&state_home).expect("remove search state");
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
fn find_rust_provider_binary(root: &std::path::Path) -> std::path::PathBuf {
    fn rust_binary(value: &serde_json::Value) -> Option<&str> {
        match value {
            serde_json::Value::Object(object) => {
                if object.get("languageId").and_then(serde_json::Value::as_str) == Some("rust") {
                    if let Some(binary) = object.get("binary").and_then(serde_json::Value::as_str) {
                        return Some(binary);
                    }
                }
                object.values().find_map(rust_binary)
            }
            serde_json::Value::Array(values) => values.iter().find_map(rust_binary),
            _ => None,
        }
    }

    fn visit(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        std::fs::read_dir(directory)
            .expect("read activation fixture directory")
            .filter_map(Result::ok)
            .for_each(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, files);
                } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    files.push(path);
                }
            });
    }

    let mut files = Vec::new();
    visit(root, &mut files);
    files.sort();
    for file in files {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(&file).expect("read activation JSON"),
        ) else {
            continue;
        };
        let Some(binary) = rust_binary(&value) else {
            continue;
        };
        let candidate = std::path::PathBuf::from(binary);
        if candidate.is_absolute() && candidate.exists() {
            return candidate;
        }
        let rooted = root.join(candidate);
        if rooted.exists() {
            return rooted;
        }
    }
    fn resolve_provider_command(
        provider: &serde_json::Map<String, serde_json::Value>,
        root: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        fn command_string(value: &serde_json::Value) -> Option<&str> {
            match value {
                serde_json::Value::String(value) => Some(value),
                serde_json::Value::Array(values) => values.first().and_then(command_string),
                serde_json::Value::Object(object) => [
                    "binary",
                    "program",
                    "executable",
                    "argv",
                    "command",
                ]
                .into_iter()
                .find_map(|key| object.get(key).and_then(command_string)),
                _ => None,
            }
        }

        ["binary", "program", "executable", "argv", "command"]
            .into_iter()
            .find_map(|key| provider.get(key).and_then(command_string))
            .map(std::path::PathBuf::from)
            .map(|path| if path.is_absolute() { path } else { root.join(path) })
    }

    fn provider_binary(
        value: &serde_json::Value,
        root: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        match value {
            serde_json::Value::Object(object) => {
                let language_is_rust = ["languageId", "language_id", "language"]
                    .into_iter()
                    .any(|key| object.get(key).and_then(serde_json::Value::as_str) == Some("rust"));
                if language_is_rust {
                    if let Some(binary) = resolve_provider_command(object, root) {
                        return Some(binary);
                    }
                }
                object
                    .values()
                    .find_map(|value| provider_binary(value, root))
            }
            serde_json::Value::Array(values) => values
                .iter()
                .find_map(|value| provider_binary(value, root)),
            _ => None,
        }
    }

    fn activation_binary(
        directory: &std::path::Path,
        root: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(directory).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(binary) = activation_binary(&path, root) {
                    return Some(binary);
                }
            } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("json") {
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                    continue;
                };
                if let Some(binary) = provider_binary(&value, root) {
                    return Some(binary);
                }
            }
        }
        None
    }

    activation_binary(root, root)
        .expect("Rust provider binary is absent from generated activation fixture")
}

fn isolate_test_provider_cwd(root: &std::path::Path) {
    let binary = find_rust_provider_binary(root);
    assert!(
        binary.starts_with(root),
        "test provider binary must be scenario-owned: {}",
        binary.display()
    );
    let wrapper = root.join(".provider-wrapper/rs-harness");
    std::fs::create_dir_all(wrapper.parent().expect("provider wrapper parent"))
        .expect("create provider wrapper directory");
    let quoted_binary = binary.to_string_lossy().replace('\'', "'\"'\"'");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprovider_cwd=\"$AST_STATE_HOME/provider-cwd\"\nmkdir -p \"$provider_cwd/src\"\ncd \"$provider_cwd\"\nexec '{quoted_binary}' \"$@\"\n"
        ),
    )
    .expect("write test provider cwd wrapper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&wrapper)
            .expect("read isolated test provider wrapper metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&wrapper, permissions)
            .expect("mark isolated test provider wrapper executable");
    }
    rewrite_test_provider_command(root, root, &binary, &wrapper);
}

fn rewrite_test_provider_command(
    directory: &std::path::Path,
    activation_root: &std::path::Path,
    binary: &std::path::Path,
    wrapper: &std::path::Path,
) {
    let absolute = binary.to_string_lossy();
    let relative = binary
        .strip_prefix(activation_root)
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    for entry in std::fs::read_dir(directory)
        .expect("read provider activation directory")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            rewrite_test_provider_command(&path, activation_root, binary, wrapper);
            continue;
        }
        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read provider activation JSON");
        let mut rewritten = text.replace(absolute.as_ref(), &wrapper.to_string_lossy());
        if let Some(relative) = &relative {
            rewritten = rewritten.replace(relative, &wrapper.to_string_lossy());
        }
        if rewritten != text {
            std::fs::write(&path, rewritten).expect("rewrite test provider activation command");
        }
    }
}
