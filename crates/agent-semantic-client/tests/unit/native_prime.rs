use std::fs;

use agent_semantic_client_core::{ClientMethod, ClientRequest, LanguageId};

use crate::native_prime::render_native_prime_seed_stdout;

#[test]
fn native_prime_seed_stdout_renders_owner_nodes_without_provider() {
    let root = temp_project_root("native-prime-seed");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(root.join("src/lib.rs"), "pub fn native_prime_gate() {}\n").expect("write src");
    let request = ClientRequest::new(ClientMethod::Search, root.clone())
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "prime".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
        ]);

    let stdout = render_native_prime_seed_stdout(&root, &request, false)
        .expect("native prime")
        .expect("native prime stdout");
    let stdout = String::from_utf8(stdout.to_vec()).expect("utf8 stdout");

    assert!(
        stdout.contains("alg=native-fd-prime-frontier-v1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("O=owner:path(src/lib.rs)!owner"),
        "{stdout}"
    );
    assert!(stdout.contains("aliases: owner:{O=owner}"), "{stdout}");
    assert!(
        !stdout.contains("aliases: graph:{G=search,O=owner}"),
        "{stdout}"
    );
    assert!(!stdout.contains("G>{O:selects}"), "{stdout}");
    assert!(!stdout.contains("rank=O frontier=O.owner"), "{stdout}");
    assert!(!stdout.contains("frontier=O.owner"), "{stdout}");
    assert!(!stdout.contains("ladder=pipe"), "{stdout}");
    assert!(!stdout.contains("next=\"asp rust search pipe"), "{stdout}");
    assert!(stdout.contains("route=evidence-state"), "{stdout}");
    assert!(
        stdout.contains("[route-graph] profile=asp-search-routing"),
        "{stdout}"
    );
    assert!(stdout.contains("chosen=UNKNOWN_WORKSPACE"), "{stdout}");
    assert!(stdout.contains("actionFrontier=A1.owner-map"), "{stdout}");
    assert!(stdout.contains("recommendedNext=A1.owner-map"), "{stdout}");
    assert!(stdout.contains("codePolicy=disabled"), "{stdout}");
    assert!(stdout.contains("line-range-selector"), "{stdout}");
    assert!(
        stdout.contains("routeOptions=\"owner-items when owner known"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=raw-read,full-json,broad-fzf"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn native_prime_seed_stdout_renders_owner_nodes_for_hidden_workspace_roots() {
    let cases = [
        ("rust", "src/lib.rs", "pub fn native_prime_gate() {}\n"),
        (
            "typescript",
            "src/index.ts",
            "export const nativePrimeGate = true;\n",
        ),
        (
            "python",
            "src/main.py",
            "def native_prime_gate():\n    return True\n",
        ),
        ("julia", "src/Main.jl", "native_prime_gate() = true\n"),
        (
            "gerbil-scheme",
            "src/std/make.ss",
            "(def (make . _) #!void)\n",
        ),
        ("org", "docs/index.org", "* Native Prime\n"),
        ("md", "docs/index.md", "# Native Prime\n"),
    ];
    let base = temp_project_root("native-prime-hidden-root");

    for (language_id, owner_path, source) in cases {
        let root = base.join(".data").join(language_id);
        let file = root.join(owner_path);
        fs::create_dir_all(file.parent().expect("fixture parent")).expect("create fixture parent");
        fs::write(&file, source).expect("write fixture source");
        let request = ClientRequest::new(ClientMethod::Search, root.clone())
            .with_language(LanguageId::from(language_id))
            .with_forwarded_args(vec![
                "prime".to_string(),
                "--view".to_string(),
                "seeds".to_string(),
            ]);

        let stdout = render_native_prime_seed_stdout(&root, &request, false)
            .expect("native prime")
            .expect("native prime stdout");
        let stdout = String::from_utf8(stdout.to_vec()).expect("utf8 stdout");

        assert!(
            stdout.contains(&format!("O=owner:path({owner_path})!owner")),
            "language={language_id} stdout={stdout}"
        );
        assert!(
            stdout.contains("aliases: owner:{O=owner}"),
            "language={language_id} stdout={stdout}"
        );
        assert!(
            !stdout.contains("aliases: graph:{G=search,O=owner}"),
            "language={language_id} stdout={stdout}"
        );
        assert!(
            !stdout.contains("frontier=O.owner"),
            "language={language_id} stdout={stdout}"
        );
        assert!(
            !stdout.contains("G>{}"),
            "language={language_id} stdout={stdout}"
        );
        assert!(
            !stdout.contains("G>{O:selects}"),
            "language={language_id} stdout={stdout}"
        );
        assert!(
            !stdout.contains("rank=O frontier=O.owner"),
            "language={language_id} stdout={stdout}"
        );
    }

    let _ = fs::remove_dir_all(base);
}

#[test]
fn native_prime_seed_stdout_does_not_intercept_json_requests() {
    let root = temp_project_root("native-prime-json");
    let request = ClientRequest::new(ClientMethod::Search, root.clone())
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "prime".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
        ]);

    let stdout = render_native_prime_seed_stdout(&root, &request, true).expect("native prime json");

    assert!(stdout.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn native_prime_seed_stdout_does_not_intercept_non_prime_search() {
    let root = temp_project_root("native-prime-non-prime");
    let request = ClientRequest::new(ClientMethod::Search, root.clone())
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "pipe".to_string(),
            "native".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
        ]);

    let stdout =
        render_native_prime_seed_stdout(&root, &request, false).expect("native prime non-prime");

    assert!(stdout.is_none());
    let _ = fs::remove_dir_all(root);
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "asp-client-native-prime-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp project root");
    root
}
