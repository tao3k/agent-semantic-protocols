use std::io::Write;
use std::process::Stdio;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_marker_provider, write_stdin_provider,
};

#[test]
fn empty_search_ingest_seeds_is_facade_diagnostic_for_all_languages() {
    let root = temp_project_root("empty-ingest-facade-diagnostic");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let providers = [
        ("rust", "rs-harness"),
        ("typescript", "ts-harness"),
        ("python", "py-harness"),
        ("julia", "asp-julia-harness"),
    ];
    for (_, binary) in providers {
        write_marker_provider(&bin_dir, binary, &marker);
    }
    write_activation(
        &root,
        &[
            provider(
                "rust",
                vec![bin_dir.join("rs-harness").display().to_string()],
            ),
            provider(
                "typescript",
                vec![bin_dir.join("ts-harness").display().to_string()],
            ),
            provider(
                "python",
                vec![bin_dir.join("py-harness").display().to_string()],
            ),
            provider(
                "julia",
                vec![bin_dir.join("asp-julia-harness").display().to_string()],
            ),
        ],
    );

    for (language, _) in providers {
        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args([
                language, "search", "ingest", "items", "tests", "--view", "seeds", ".",
            ])
            .output()
            .expect("run empty ingest facade");

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(stdout.starts_with("[search-ingest]"));
        assert!(stdout.contains("|note kind=stdin-required"));
        assert!(stdout.contains("search prime --view seeds"));
        assert!(stdout.contains("|next prime:"));
        assert!(!stdout.contains("test:path(.)"));
        assert!(!stdout.contains("owner:path(search prime"));
        assert!(
            !marker.exists(),
            "empty ingest should not spawn provider for {language}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_stdin_is_preserved_for_pipe_commands() {
    let root = temp_project_root("provider-stdin-facade");
    let bin_dir = root.join(".bin");
    write_stdin_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let mut child = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rust", "search", "ingest", "."])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp rust search ingest");
    child
        .stdin
        .as_mut()
        .expect("facade stdin")
        .write_all(b"src/lib.rs:10:HookDecision\n")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for facade");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "stdin=src/lib.rs:10:HookDecision\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_is_asp_owned_and_renders_generated_candidates_without_provider_spawn() {
    let root = temp_project_root("search-pipe-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct HookDecision;\npub struct ClientReceipt;\nfn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision ClientReceipt",
            "--pipe",
            "items,tests",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-ingest]"), "{stdout}");
    assert!(
        stdout.contains("O=owner:path(src/lib.rs)!owner"),
        "{stdout}"
    );
    assert!(
        stdout.contains("S=symbol:symbol(hookdecision)@src/lib.rs:1:1!symbol"),
        "{stdout}"
    );
    assert!(
        stdout.contains("S2=symbol:symbol(clientreceipt)@src/lib.rs:2:2!symbol"),
        "{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_commands_view_does_not_spawn_provider() {
    let root = temp_project_root("search-pipe-commands-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision",
            "--pipe",
            "items,tests",
            "--view",
            "commands",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe commands");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-pipe]"), "{stdout}");
    assert!(stdout.contains("|replace slow="), "{stdout}");
    assert!(!marker.exists(), "commands view should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reasoning_owner_query_is_asp_owned_and_does_not_spawn_provider() {
    let root = temp_project_root("search-reasoning-owner-query-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "fn unrelated() {}\nfn render_fast_prime_search() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "reasoning",
            "owner-query",
            "--owner",
            "src/lib.rs",
            "--query",
            "render_fast_prime_search",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search reasoning owner-query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-reasoning]"), "{stdout}");
    assert!(
        stdout.contains("I=item:symbol(render_fast_prime_search)@src/lib.rs:2:2!code"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "owner-query fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
