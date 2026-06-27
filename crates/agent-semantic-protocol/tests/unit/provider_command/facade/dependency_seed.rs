use std::time::{Duration, Instant};

use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, temp_project_root, write_activation,
};

const DIRECT_DEPENDENCY_SEED_GATE: Duration = Duration::from_millis(500);

#[test]
fn direct_dependency_seed_falls_back_to_asp_manifest_without_topology_capability() {
    let cases = [
        DependencySeedCase {
            language: "typescript",
            binary: "ts-harness",
            query: "react",
            manifest_path: "package.json",
            manifest_text: r#"{"dependencies":{"react":"18.2.0"}}"#,
            expected_requirement: "18.2.0",
        },
        DependencySeedCase {
            language: "python",
            binary: "py-harness",
            query: "requests",
            manifest_path: "pyproject.toml",
            manifest_text: "[project]\nname = \"dep-seed-python\"\nversion = \"0.1.0\"\ndependencies = [\"requests>=2.31\"]\n",
            expected_requirement: ">=2.31",
        },
        DependencySeedCase {
            language: "julia",
            binary: "asp-julia-harness",
            query: "DataFrames",
            manifest_path: "Project.toml",
            manifest_text: "[deps]\nDataFrames = \"a93c6f00-e57d-5684-b7b6-d8193f3e46c0\"\n[compat]\nDataFrames = \"1.6\"\n",
            expected_requirement: "1.6",
        },
        DependencySeedCase {
            language: "gerbil-scheme",
            binary: "gslph",
            query: "git.cons.io/mighty-gerbils/gerbil-poo",
            manifest_path: "gerbil.pkg",
            manifest_text: "(package: dep-seed-gerbil depend: \"git.cons.io/mighty-gerbils/gerbil-poo\")\n",
            expected_requirement: "-",
        },
    ];

    for case in cases {
        case.assert_fast_dependency_seed();
    }
}

#[test]
fn direct_dependency_seed_uses_provider_dependency_topology_when_available() {
    let root = temp_project_root("direct-dependency-seed-provider-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-rust\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    let manifest_hash = {
        let bytes = std::fs::read(root.join("Cargo.toml")).expect("read manifest");
        let digest = <sha2::Sha256 as sha2::Digest>::digest(&bytes);
        format!("sha256:{digest:x}")
    };
    let provider_path = bin_dir.join("rs-harness");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\ncase \"$*\" in\n  *dependency-topology-metadata*)\n    cat <<'JSON'\n{{\"packetKind\":\"dependency-topology-metadata\",\"fingerprint\":\"sha256:1111111111111111111111111111111111111111111111111111111111111111\",\"cacheKey\":{{\"languageId\":\"rust\",\"packageManager\":\"cargo\",\"manifestHash\":\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"lockfileHash\":\"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"projectPackageName\":\"dep-seed-rust\"}}}}\nJSON\n    ;;\n  *)\n    cat <<'JSON'\n{{\"packetKind\":\"dependency-topology\",\"fingerprint\":\"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\",\"cacheKey\":{{\"languageId\":\"rust\",\"packageManager\":\"cargo\",\"manifestHash\":\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"lockfileHash\":\"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"projectPackageName\":\"dep-seed-rust\"}},\"graph\":{{\"nodes\":[{{\"id\":\"dependency:serde\",\"kind\":\"dependency\",\"value\":\"serde\",\"path\":\"Cargo.toml\",\"fields\":{{\"dependencyName\":\"serde\",\"manifestPath\":\"Cargo.toml\"}}}},{{\"id\":\"dependency-version:serde\",\"kind\":\"dependency-version\",\"value\":\"1\",\"fields\":{{\"version\":\"1\"}}}}],\"edges\":[{{\"source\":\"dependency:serde\",\"target\":\"dependency-version:serde\",\"relation\":\"version_locked\"}}]}}}}\nJSON\n    ;;\nesac\n",
            marker.display()
        )
        .replace(
            "\"graph\":",
            &format!(
                "\"sources\":{{\"manifests\":[{{\"path\":\"Cargo.toml\",\"sha256\":\"{manifest_hash}\"}}],\"lockfiles\":[],\"usageSites\":[]}},\"graph\":"
            ),
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let first = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "search",
            "deps",
            "serde",
            "--workspace",
            ".",
            "--view",
            "hits",
        ])
        .output()
        .expect("run provider dependency topology seed");
    assert!(
        first.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        marker.exists(),
        "dependency seed should spawn topology provider"
    );
    let calls = std::fs::read_to_string(&marker).expect("read provider calls");
    assert!(calls.contains("dependency-topology-metadata"), "{calls}");
    assert!(calls.contains("dependency-topology"), "{calls}");
    let stdout = String::from_utf8(first.stdout).expect("stdout");
    assert!(stdout.contains("topology=provider-owned"), "{stdout}");
    assert!(stdout.contains("seedCache=miss"), "{stdout}");
    assert!(stdout.contains("requirement=\"1\""), "{stdout}");
    let namespaced_seed_cache = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("search")
        .join("dependency-seeds")
        .join("rust.tsv");
    let legacy_seed_cache = root
        .join(".cache")
        .join("search")
        .join("dependency-seeds")
        .join("rust.tsv");
    assert!(
        namespaced_seed_cache.exists(),
        "dependency seed cache should live under ASP state/cache namespace"
    );
    assert!(
        !legacy_seed_cache.exists(),
        "dependency seed cache must not be written under project-local .cache/search"
    );

    let second = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "search",
            "deps",
            "serde",
            "--workspace",
            ".",
            "--view",
            "hits",
        ])
        .output()
        .expect("rerun provider dependency topology seed");
    assert!(
        second.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8(second.stdout).expect("stdout");
    assert!(stdout.contains("topology=provider-owned"), "{stdout}");
    assert!(stdout.contains("seedCache=hit"), "{stdout}");
    let calls = std::fs::read_to_string(&marker).expect("read provider calls");
    assert_eq!(
        calls.matches("dependency-topology-metadata").count(),
        1,
        "{calls}"
    );
    assert_eq!(
        calls.matches("dependency-topology --json").count(),
        1,
        "{calls}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn direct_dependency_seed_resolves_provider_bin_from_activation_root_for_external_workspace() {
    let activation_root = temp_project_root("direct-dependency-seed-provider-bin-activation-root");
    let external_root = temp_project_root("direct-dependency-seed-provider-bin-external-root");
    let bin_dir = activation_root.join(".bin");
    let marker = activation_root.join("provider-called");
    std::fs::write(
        activation_root.join("asp.toml"),
        "[languages.rust]\nbin = \".bin/rs-harness\"\n",
    )
    .expect("write asp.toml");
    std::fs::write(
        external_root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-rust-external\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntokio = \"1\"\n",
    )
    .expect("write external Cargo.toml");
    let provider_path = bin_dir.join("rs-harness");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\ncat <<'JSON'\n{{\"packetKind\":\"dependency-topology\",\"fingerprint\":\"sha256:2222222222222222222222222222222222222222222222222222222222222222\",\"graph\":{{\"nodes\":[{{\"id\":\"dependency:tokio\",\"kind\":\"dependency\",\"value\":\"tokio\",\"path\":\"Cargo.toml\",\"fields\":{{\"dependencyName\":\"tokio\",\"manifestPath\":\"Cargo.toml\"}}}},{{\"id\":\"dependency-version:tokio\",\"kind\":\"dependency-version\",\"value\":\"1\",\"fields\":{{\"version\":\"1\"}}}}],\"edges\":[{{\"source\":\"dependency:tokio\",\"target\":\"dependency-version:tokio\",\"relation\":\"version_locked\"}}]}}}}\nJSON\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&activation_root, &[provider("rust", Vec::new())]);

    assert!(
        !external_root.join(".bin").exists(),
        "external workspace must not provide the configured provider bin"
    );
    let output = asp_command(&activation_root)
        .args([
            "rust",
            "search",
            "deps",
            "tokio",
            "--workspace",
            external_root.to_str().expect("external root path"),
            "--view",
            "hits",
        ])
        .output()
        .expect("run external provider dependency topology seed");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        marker.exists(),
        "dependency seed should spawn activation-root provider"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("topology=provider-owned"), "{stdout}");
    assert!(stdout.contains("seedCache=miss"), "{stdout}");
    assert!(stdout.contains("|dependency D:tokio"), "{stdout}");
    assert!(stdout.contains("requirement=\"1\""), "{stdout}");

    let _ = std::fs::remove_dir_all(activation_root);
    let _ = std::fs::remove_dir_all(external_root);
}

#[test]
fn direct_dependency_seed_no_hit_does_not_dump_full_manifest() {
    let root = temp_project_root("direct-dependency-seed-no-hit");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-no-hit\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "rust",
            "search",
            "deps",
            "tokio",
            "--workspace",
            ".",
            "--view",
            "hits",
        ])
        .output()
        .expect("run direct dependency seed no hit");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("q=tokio"), "{stdout}");
    assert!(stdout.contains("manifest=0"), "{stdout}");
    assert!(stdout.contains("hit=0"), "{stdout}");
    assert!(!stdout.contains("|dependency D:serde"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn direct_dependency_seed_rejects_extra_positional_api_token() {
    let root = temp_project_root("direct-dependency-seed-extra-positional");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-extra-positional\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntokio = \"1\"\n",
    )
    .expect("write Cargo.toml");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "rust",
            "search",
            "deps",
            "tokio",
            "spawn",
            "--workspace",
            ".",
            "--view",
            "hits",
        ])
        .output()
        .expect("run direct dependency seed with extra positional");

    assert!(
        !output.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("search deps accepts one dependency selector"),
        "{stderr}"
    );
    assert!(stderr.contains("search deps tokio::spawn"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn direct_dependency_seed_api_selector_emits_local_usage_frontier() {
    let root = temp_project_root("direct-dependency-seed-api-selector");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-api-selector\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntokio = \"1\"\n",
    )
    .expect("write Cargo.toml");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let stdout = run_dependency_seed_stdout(&root, "tokio@1::spawn");
    assert!(stdout.contains("q=tokio@1::spawn"), "{stdout}");
    assert!(stdout.contains("apiQuery=spawn"), "{stdout}");
    assert!(stdout.contains("|dependency D:tokio"), "{stdout}");
    assert!(stdout.contains("docs-use:tokio@1::spawn"), "{stdout}");
    assert!(stdout.contains("crate-source:tokio"), "{stdout}");
    assert!(stdout.contains("import:tokio"), "{stdout}");
    assert!(stdout.contains("tests:spawn"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn direct_dependency_seed_api_selector_expands_rust_path_dependency_public_api() {
    let root = temp_project_root("direct-dependency-seed-api-selector-public-api");
    let dependency_root = root.join("rust-lang-project-harness");
    std::fs::create_dir_all(root.join("src")).expect("create root src");
    std::fs::create_dir_all(dependency_root.join("src")).expect("create dependency src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-api-selector-public-api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nrust-lang-project-harness = { path = \"rust-lang-project-harness\" }\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(root.join("src/lib.rs"), "pub fn root() {}\n").expect("write root lib");
    std::fs::write(
        dependency_root.join("Cargo.toml"),
        "[package]\nname = \"rust-lang-project-harness\"\nversion = \"0.1.2\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n",
    )
    .expect("write dependency Cargo.toml");
    std::fs::write(
        dependency_root.join("src/lib.rs"),
        "pub struct Scenario;\nstruct InternalScenario;\npub fn performance_gate() {}\n",
    )
    .expect("write dependency lib");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "rust",
            "search",
            "deps",
            "rust-lang-project-harness::Scenario",
            "--workspace",
            ".",
            "--view",
            "public-external-types",
        ])
        .output()
        .expect("run direct dependency seed API expansion");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("|external dependency=rust-lang-project-harness"),
        "{stdout}"
    );
    assert!(stdout.contains("source=cargo-metadata"), "{stdout}");
    assert!(
        stdout.contains("|external-api name=Scenario kind=struct"),
        "{stdout}"
    );
    assert!(stdout.contains("match=exact"), "{stdout}");
    assert!(stdout.contains("src/lib.rs:1-1"), "{stdout}");
    assert!(
        stdout.contains("asp rust query --selector"),
        "agent should receive a concrete selector command; stdout={stdout}"
    );
    assert!(
        !stdout.contains("InternalScenario"),
        "private dependency item must not be exposed; stdout={stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn direct_dependency_seed_reuses_cached_manifest_topology_until_manifest_changes() {
    let root = temp_project_root("direct-dependency-seed-cache");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-cache\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let first = run_dependency_seed_stdout(&root, "serde");
    assert!(first.contains("seedCache=miss"), "{first}");
    assert!(first.contains("requirement=\"^1\""), "{first}");

    let second = run_dependency_seed_stdout(&root, "serde");
    assert!(second.contains("seedCache=hit"), "{second}");
    assert!(second.contains("requirement=\"^1\""), "{second}");

    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-cache\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\ntokio = \"1\"\n",
    )
    .expect("update Cargo.toml");

    let third = run_dependency_seed_stdout(&root, "tokio");
    assert!(third.contains("seedCache=miss"), "{third}");
    assert!(third.contains("|dependency D:tokio"), "{third}");
    assert!(third.contains("requirement=\"^1\""), "{third}");

    let fourth = run_dependency_seed_stdout(&root, "tokio");
    assert!(fourth.contains("seedCache=hit"), "{fourth}");
    assert!(fourth.contains("|dependency D:tokio"), "{fourth}");

    let _ = std::fs::remove_dir_all(root);
}

fn run_dependency_seed_stdout(root: &std::path::Path, query: &str) -> String {
    let output = asp_command(root)
        .args([
            "rust",
            "search",
            "deps",
            query,
            "--workspace",
            ".",
            "--view",
            "hits",
        ])
        .output()
        .expect("run direct dependency seed");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout")
}

struct DependencySeedCase {
    language: &'static str,
    binary: &'static str,
    query: &'static str,
    manifest_path: &'static str,
    manifest_text: &'static str,
    expected_requirement: &'static str,
}

impl DependencySeedCase {
    fn assert_fast_dependency_seed(&self) {
        let root = temp_project_root(&format!("direct-dependency-seed-{}", self.language));
        let bin_dir = root.join(".bin");
        let marker = root.join("provider-called");
        std::fs::create_dir_all(&bin_dir).expect("create bin dir");
        std::fs::write(root.join(self.manifest_path), self.manifest_text).expect("write manifest");
        let provider_path = bin_dir.join(self.binary);
        std::fs::write(
            &provider_path,
            format!("#!/bin/sh\nprintf called > '{}'\n", marker.display()),
        )
        .expect("write provider");
        make_executable(&provider_path);
        write_activation(&root, &[provider(self.language, Vec::new())]);

        let warmup = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .args([
                self.language,
                "search",
                "deps",
                self.query,
                "--workspace",
                ".",
                "--view",
                "hits",
            ])
            .output()
            .expect("warm direct dependency seed");
        assert!(
            warmup.status.success(),
            "language={} stderr={}",
            self.language,
            String::from_utf8_lossy(&warmup.stderr)
        );
        assert!(
            !marker.exists(),
            "language={} dependency seed should not spawn provider",
            self.language
        );

        let started_at = Instant::now();
        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .args([
                self.language,
                "search",
                "deps",
                self.query,
                "--workspace",
                ".",
                "--view",
                "hits",
            ])
            .output()
            .expect("run direct dependency seed");
        let elapsed = started_at.elapsed();
        assert!(
            output.status.success(),
            "language={} stderr={}",
            self.language,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            elapsed < DIRECT_DEPENDENCY_SEED_GATE,
            "language={} dependency seed exceeded {:?}; elapsed={elapsed:?}",
            self.language,
            DIRECT_DEPENDENCY_SEED_GATE
        );
        assert!(
            !marker.exists(),
            "language={} dependency seed should not spawn provider",
            self.language
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(stdout.contains("[search-deps]"), "{stdout}");
        assert!(stdout.contains("usage=0"), "{stdout}");
        assert!(stdout.contains("topology=asp-owned"), "{stdout}");
        assert!(stdout.contains("source=manifest"), "{stdout}");
        assert!(
            stdout.contains(&format!("requirement=\"{}\"", self.expected_requirement)),
            "{stdout}"
        );
        assert!(stdout.contains("topology=asp-owned"), "{stdout}");

        let _ = std::fs::remove_dir_all(root);
    }
}
