use std::fs;

use agent_semantic_client_core::{ClientMethod, ClientRequest, LanguageId};

use crate::native_prime::render_native_prime_seed_stdout;

#[test]
fn native_prime_seed_stdout_renders_owner_frontier_without_provider() {
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
    assert!(
        stdout.contains("next=\"asp rust search pipe '<question-or-feature-term>'"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=raw-read,full-json,broad-fzf"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn native_prime_seed_stdout_renders_owner_frontier_for_hidden_workspace_root() {
    let base = temp_project_root("native-prime-hidden-root");
    let root = base.join(".data").join("gerbil");
    fs::create_dir_all(root.join("src/std")).expect("create gerbil std");
    fs::write(root.join("src/std/make.ss"), "(def (make . _) #!void)\n").expect("write make");
    let request = ClientRequest::new(ClientMethod::Search, root.clone())
        .with_language(LanguageId::from("gerbil-scheme"))
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
        stdout.contains("O=owner:path(src/std/make.ss)!owner"),
        "{stdout}"
    );
    assert!(!stdout.contains("G>{}"), "{stdout}");
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
