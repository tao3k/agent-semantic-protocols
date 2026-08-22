use super::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn missing_provider_binary_is_reported_before_provider_spawn() {
    let root = temp_project_root("missing-activation");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"missing-activation\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/lib.rs"), "").expect("write lib.rs");

    let output = asp_command(&root)
        .env_remove("PATH")
        .args(["rust", "guide"])
        .output()
        .expect("run asp rust search");

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to sync generated activation")
            || stderr.contains("expected State Home runtime bin"),
        "{stderr}"
    );
    assert!(
        stderr.contains("expected State Home runtime bin")
            || stderr.contains("run `asp install plugin --codex"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn diagnostic_commands_do_not_require_activation() {
    let root = temp_project_root("diagnostics-without-activation");
    for args in [
        vec!["guide"],
        vec!["doctor"],
        vec!["providers", "list"],
        vec!["cache", "status"],
    ] {
        let output = super::runtime_support::runtime_client_command(&root)
            .args(&args)
            .output()
            .expect("run asp diagnostic command");
        assert!(output.status.success(), "{args:?}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        match args.as_slice() {
            ["guide"] => assert!(stdout.contains("[asp-guide]"), "{stdout}"),
            ["doctor"] => {
                assert!(stdout.contains("[asp-doctor] status="), "{stdout}");
                assert!(
                    stdout.contains("status=degraded") || stdout.contains("status=ok"),
                    "{stdout}"
                );
                if stdout.contains("status=degraded") {
                    assert!(stdout.contains("activation=missing"), "{stdout}");
                    assert!(
                        stdout.contains("|cmd install=asp install plugin --codex ."),
                        "{stdout}"
                    );
                } else {
                    assert!(stdout.contains("providers="), "{stdout}");
                    assert!(stdout.contains("server=not-required"), "{stdout}");
                }
            }
            ["providers", "list"] => {
                let packet: serde_json::Value =
                    serde_json::from_str(&stdout).expect("provider registry JSON");
                let providers = packet["providers"]
                    .as_array()
                    .expect("provider registry entries");
                assert!(!providers.is_empty(), "{stdout}");
                assert!(
                    providers.iter().all(|entry| {
                        entry["activation"]["status"] == "unavailable"
                            && entry["activation"]["reasonKind"] == "provider-registry-unavailable"
                    }),
                    "{stdout}"
                );
            }
            ["cache", "status"] => {
                assert!(stdout.contains("[asp-cache] status=missing"), "{stdout}");
                assert!(stdout.contains("activation="), "{stdout}");
            }
            _ => unreachable!("covered args"),
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn non_agent_command_surface_is_rejected_without_provider_spawn() {
    let root = temp_project_root("protocol-provider-errors-non-agent");
    let bin_dir = root.join(".bin");
    let called = root.join("called");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["rust", "fmt", "."])
        .output()
        .expect("run protocol");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("usage: asp <") && stderr.contains("<guide|search|query|check"),
        "{stderr}"
    );
    assert!(!called.exists(), "provider should not have been spawned");

    assert!(!called.exists(), "provider should not have been spawned");
    std::fs::remove_dir_all(root).ok();
}
#[tokio::test]
async fn provider_language_facades_route_language_search_through_runtime_provider() {
    let root = crate::workspace_tree_sitter_query_diagnostics::create_linked_fixture_workspace(
        "provider-dash-language-query",
    );
    std::fs::write(
        root.join(".gitignore"),
        "home/\n.bin/\n*.provider-invocation\n",
    )
    .expect("write provider search fixture ignore rules");
    std::fs::write(
        root.join("fixture.rs"),
        "pub fn provider_search_fixture() {}\n",
    )
    .expect("write provider search fixture source");
    let bin_dir = root.join(".bin");
    let languages = [
        ("gerbil-scheme", "gerbil-scheme-harness"),
        ("julia", "asp-julia-harness"),
        ("python", "py-harness"),
        ("rust", "rs-harness"),
        ("typescript", "asp-typescript"),
    ];
    let providers = languages
        .iter()
        .map(|&(language_id, binary)| {
            super::support::write_recording_provider(
                &root,
                &bin_dir,
                language_id,
                binary,
                &format!("{language_id}-runtime"),
                &root.join(format!("{language_id}.provider-invocation")),
            );
            provider(language_id, Vec::new())
        })
        .collect::<Vec<_>>();
    write_activation(&root, &providers);
    let staged = std::process::Command::new("git")
        .current_dir(&root)
        .args(["add", ".pre-commit-config.yaml", ".gitignore", "fixture.rs"])
        .status()
        .expect("stage provider search linked fixture");
    assert!(staged.success(), "stage provider search linked fixture");
    crate::workspace_tree_sitter_query_diagnostics::commit_linked_fixture_workspace(&root);
    super::runtime_support::publish_runtime_server_artifact(&root).await;
    let runtime_start = std::time::Instant::now();
    let runtime_server = super::support::start_runtime_server(&root);
    let runtime_start_elapsed = runtime_start.elapsed();
    eprintln!(
        "[runtime-language-provider-perf] phase=server-start elapsedMs={:.3}",
        runtime_start_elapsed.as_secs_f64() * 1_000.0
    );
    assert!(
        runtime_start_elapsed < std::time::Duration::from_secs(5),
        "Runtime Server startup exceeded 5s: elapsed={runtime_start_elapsed:?}"
    );
    super::runtime_support::admit_runtime_resident_generation(&root);
    for (language_id, _) in languages {
        let provider_marker_path = root.join(format!("{language_id}.provider-invocation"));
        let provider_marker_before_query = std::fs::read(&provider_marker_path).ok();
        let resident = super::runtime_support::run_resident_search(
            &root,
            language_id,
            &[
                "search", "lexical", "--query", "owner", "--query", "tests", "--view", "seeds",
            ],
        );
        eprintln!(
            "[runtime-language-provider-perf] phase=resident-query languageId={language_id} elapsedMicros={} residentReadMicros={} serviceMicros={} readState={:?} candidates={}",
            resident.elapsed_micros,
            resident.resident_read_elapsed_micros,
            resident.service_elapsed_micros,
            resident.read_state,
            resident.candidate_count
        );
        assert_eq!(resident.status_code, 0, "languageId={language_id}");
        assert!(
            resident.elapsed_micros < 1_000,
            "Runtime resident provider query exceeded 1ms: languageId={language_id} elapsedMicros={}",
            resident.elapsed_micros
        );
        assert!(
            matches!(
                resident.read_state,
                agent_semantic_client_db::ClientDbSourceIndexLookupState::Hit
                    | agent_semantic_client_db::ClientDbSourceIndexLookupState::Miss
                    | agent_semantic_client_db::ClientDbSourceIndexLookupState::EmptyIndex
            ),
            "languageId={language_id} readState={:?}",
            resident.read_state
        );
        let output = asp_command(&root)
            .args([
                language_id,
                "search",
                "lexical",
                "--query",
                "owner",
                "--query",
                "tests",
                "--workspace",
                ".",
                "--view",
                "seeds",
            ])
            .output()
            .expect("run ASP language search through Runtime Server");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "languageId={language_id} stderr={stderr}\nstdout={}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert_eq!(
            std::fs::read(&provider_marker_path).ok(),
            provider_marker_before_query,
            "languageId={language_id} query invoked a provider process; stdout={stdout} stderr={stderr}"
        );
        let repeated = super::runtime_support::run_resident_search(
            &root,
            language_id,
            &[
                "search", "lexical", "--query", "owner", "--query", "tests", "--view", "seeds",
            ],
        );
        eprintln!(
            "[runtime-language-provider-perf] phase=resident-repeat languageId={language_id} elapsedMicros={} residentReadMicros={} serviceMicros={} readState={:?} candidates={}",
            repeated.elapsed_micros,
            repeated.resident_read_elapsed_micros,
            repeated.service_elapsed_micros,
            repeated.read_state,
            repeated.candidate_count
        );
        assert!(
            repeated.elapsed_micros < 1_000,
            "Repeated Runtime resident provider query exceeded 1ms: languageId={language_id} elapsedMicros={}",
            repeated.elapsed_micros
        );
        assert_eq!(repeated.stdout, resident.stdout, "languageId={language_id}");
        assert_eq!(
            std::fs::read(&provider_marker_path).ok(),
            provider_marker_before_query,
            "languageId={language_id} repeated query invoked a provider process"
        );
    }
    let output = asp_command(&root)
        .args(["check", "--language", "rust", "."])
        .output()
        .expect("run asp check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("asp check is not a public command surface"),
        "{stderr}"
    );
    assert!(
        stderr.contains("use asp <rust|typescript|python|julia> check"),
        "{stderr}"
    );
    drop(runtime_server);
    let removed = std::process::Command::new("git")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["worktree", "remove", "--force"])
        .arg(&root)
        .status()
        .expect("remove provider search linked fixture");
    assert!(removed.success(), "remove provider search linked fixture");
}
