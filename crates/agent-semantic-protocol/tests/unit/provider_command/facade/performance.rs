use std::time::{Duration, Instant};

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
};

const ASP_FACADE_PERFORMANCE_GATE: Duration = Duration::from_secs(3);
const JULIA_FACADE_PERFORMANCE_GATE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
struct FacadePerformanceProvider {
    language: &'static str,
    binary: &'static str,
    label: &'static str,
    owner: &'static str,
    query: &'static str,
}

#[test]
fn language_facade_regular_commands_finish_inside_performance_gate() {
    let root = temp_project_root("language-facade-performance-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let providers = [
        FacadePerformanceProvider {
            language: "rust",
            binary: "rs-harness",
            label: "rs",
            owner: "src/lib.rs",
            query: "RustGate",
        },
        FacadePerformanceProvider {
            language: "typescript",
            binary: "ts-harness",
            label: "ts",
            owner: "src/index.ts",
            query: "typescriptGate",
        },
        FacadePerformanceProvider {
            language: "python",
            binary: "py-harness",
            label: "py",
            owner: "src/main.py",
            query: "python_gate",
        },
        FacadePerformanceProvider {
            language: "julia",
            binary: "asp-julia-harness",
            label: "julia",
            owner: "src/main.jl",
            query: "julia_gate",
        },
        FacadePerformanceProvider {
            language: "gerbil-scheme",
            binary: "gerbil-scheme-harness",
            label: "gerbil",
            owner: "src/main.ss",
            query: "gerbil-gate",
        },
    ];
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_regular_search_fixtures(&root);
    for provider in providers {
        write_echo_provider(&bin_dir, provider.binary, provider.label);
    }
    write_activation(
        &root,
        &providers
            .iter()
            .map(|provider_config| {
                provider(
                    provider_config.language,
                    vec![bin_dir.join(provider_config.binary).display().to_string()],
                )
            })
            .collect::<Vec<_>>(),
    );

    for provider in providers {
        let command_suite = [
            vec![
                provider.language,
                "query",
                provider.owner,
                "--query",
                provider.query,
                ".",
            ],
            vec![provider.language, "search", "prime", "--view", "seeds", "."],
            vec![
                provider.language,
                "search",
                "pipe",
                provider.query,
                "--view",
                "seeds",
                ".",
            ],
            vec![
                provider.language,
                "search",
                "pipe",
                provider.query,
                "--view",
                "graph-turbo-request",
                ".",
            ],
        ];
        for args in command_suite {
            let warmup = asp_command(&root)
                .env("PATH", prepend_path(&bin_dir))
                .env("PRJ_CACHE_HOME", &cache_home)
                .args(&args)
                .output()
                .unwrap_or_else(|error| panic!("warm asp {args:?}: {error}"));
            assert!(
                warmup.status.success(),
                "warm args={args:?} stderr={}",
                String::from_utf8_lossy(&warmup.stderr)
            );

            let started_at = Instant::now();
            let output = asp_command(&root)
                .env("PATH", prepend_path(&bin_dir))
                .env("PRJ_CACHE_HOME", &cache_home)
                .args(&args)
                .output()
                .unwrap_or_else(|error| panic!("run asp {args:?}: {error}"));
            let elapsed = started_at.elapsed();
            assert!(
                output.status.success(),
                "args={args:?} stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            let gate = performance_gate_for_language(provider.language);
            assert!(
                elapsed < gate,
                "asp {args:?} exceeded {gate:?}; elapsed={elapsed:?}; stdout={}; stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).expect("stdout");
            assert_regular_command_output(&args, &stdout, provider.label);
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

fn performance_gate_for_language(language: &str) -> Duration {
    if language == "julia" {
        JULIA_FACADE_PERFORMANCE_GATE
    } else {
        ASP_FACADE_PERFORMANCE_GATE
    }
}

fn assert_regular_command_output(args: &[&str], stdout: &str, label: &str) {
    if matches!(args.get(1), Some(&"query")) {
        assert!(
            stdout.contains(&format!("{label} args="))
                || stdout.contains("reason=owner-not-found")
                || stdout.contains("[search-owner]"),
            "args={args:?} stdout={stdout}"
        );
        return;
    }
    if matches!(args.get(1..3), Some(["search", "prime"])) {
        assert!(
            stdout.contains("[search-prime]") || stdout.contains(&format!("{label} args=")),
            "args={args:?} stdout={stdout}"
        );
        return;
    }
    if matches!(args.get(1..3), Some(["search", "pipe"])) && args.contains(&"graph-turbo-request") {
        let payload: serde_json::Value = serde_json::from_str(stdout)
            .unwrap_or_else(|error| panic!("args={args:?} graph request json: {error}; {stdout}"));
        assert_eq!(
            payload["packetKind"].as_str(),
            Some("graph-turbo-request"),
            "{payload}"
        );
        return;
    }
    assert!(
        stdout.contains("[search-pipe]"),
        "args={args:?} stdout={stdout}"
    );
}

fn write_regular_search_fixtures(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub struct RustGate;\n").expect("write rust");
    std::fs::write(
        root.join("src/index.ts"),
        "export const typescriptGate = 1;\n",
    )
    .expect("write ts");
    std::fs::write(
        root.join("src/main.py"),
        "def python_gate():\n    return 1\n",
    )
    .expect("write python");
    std::fs::write(root.join("src/main.jl"), "const julia_gate = 1\n").expect("write julia");
    std::fs::write(root.join("src/main.ss"), "(export gerbil-gate)\n").expect("write gerbil");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"regular-gate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(root.join("package.json"), "{\"name\":\"regular-gate\"}\n")
        .expect("write package.json");
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"regular-gate\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject.toml");
    std::fs::write(root.join("Project.toml"), "name = \"regular-gate\"\n")
        .expect("write Project.toml");
    std::fs::write(root.join("gerbil.pkg"), "(package: regular-gate)\n").expect("write gerbil.pkg");
}

#[test]
fn dependency_manifest_graph_requests_finish_inside_performance_gate() {
    let root = temp_project_root("dependency-manifest-performance-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let providers = [
        ("rust", "rs-harness", "rs"),
        ("typescript", "ts-harness", "ts"),
        ("python", "py-harness", "py"),
        ("julia", "asp-julia-harness", "julia"),
        ("gerbil-scheme", "gerbil-scheme-harness", "gerbil"),
    ];
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_dependency_manifest_fixtures(&root);
    for (_, binary, label) in providers.iter().copied() {
        write_echo_provider(&bin_dir, binary, label);
    }
    write_activation(
        &root,
        &providers
            .iter()
            .map(|(language, binary, _)| {
                provider(language, vec![bin_dir.join(binary).display().to_string()])
            })
            .collect::<Vec<_>>(),
    );

    for (language, _, _) in providers.iter().copied() {
        let args = [
            language,
            "search",
            "pipe",
            "dep159",
            "--view",
            "graph-turbo-request",
            ".",
        ];
        let warmup = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", &cache_home)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("warm asp {args:?}: {error}"));
        assert!(
            warmup.status.success(),
            "warm args={args:?} stderr={}",
            String::from_utf8_lossy(&warmup.stderr)
        );

        let started_at = Instant::now();
        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", &cache_home)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run asp {args:?}: {error}"));
        let elapsed = started_at.elapsed();
        assert!(
            output.status.success(),
            "args={args:?} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            elapsed < ASP_FACADE_PERFORMANCE_GATE,
            "asp {args:?} exceeded {ASP_FACADE_PERFORMANCE_GATE:?}; elapsed={elapsed:?}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let payload: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("graph request json");
        assert!(
            payload["graph"]["nodes"].as_array().is_some_and(|nodes| {
                nodes.iter().any(|node| {
                    node["kind"].as_str() == Some("dependency")
                        && node["value"].as_str() == Some("dep159")
                        && node["confidence"].as_str() == Some("exact")
                })
            }),
            "{payload}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

fn write_dependency_manifest_fixtures(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub struct DependencyGate;\n").expect("write rust");
    std::fs::write(
        root.join("src/index.ts"),
        "export const dependencyGate = 1;\n",
    )
    .expect("write ts");
    std::fs::write(root.join("src/main.py"), "dependency_gate = 1\n").expect("write python");
    std::fs::write(root.join("src/main.jl"), "const dependency_gate = 1\n").expect("write julia");
    std::fs::write(root.join("src/main.ss"), "(export dependency-gate)\n").expect("write gerbil");
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"dependency-gate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{}",
            (0..160)
                .map(|index| format!("dep{index} = \"1.{index}.0\"\n"))
                .collect::<String>()
        ),
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("package.json"),
        format!(
            "{{\n  \"dependencies\": {{\n{}\n  }}\n}}\n",
            (0..160)
                .map(|index| {
                    let suffix = if index == 159 { "" } else { "," };
                    format!("    \"dep{index}\": \"1.{index}.0\"{suffix}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .expect("write package.json");
    std::fs::write(
        root.join("pyproject.toml"),
        format!(
            "[project]\nname = \"dependency-gate\"\nversion = \"0.1.0\"\ndependencies = [\n{}\n]\n",
            (0..160)
                .map(|index| format!("  \"dep{index}>=1.{index}.0\","))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .expect("write pyproject.toml");
    std::fs::write(
        root.join("Project.toml"),
        format!(
            "[deps]\n{}",
            (0..160)
                .map(|index| format!("dep{index} = \"00000000-0000-0000-0000-{index:012}\"\n"))
                .collect::<String>()
        ),
    )
    .expect("write Project.toml");
    std::fs::write(
        root.join("Manifest.toml"),
        (0..160)
            .map(|index| {
                format!(
                    "[[deps.dep{index}]]\nuuid = \"00000000-0000-0000-0000-{index:012}\"\nversion = \"1.{index}.0\"\n"
                )
            })
            .collect::<String>(),
    )
    .expect("write Manifest.toml");
    std::fs::write(
        root.join("gerbil.pkg"),
        format!(
            "(package: dependency-gate\n{})\n",
            (0..160)
                .map(|index| format!(" depend: (\"dep{index}\")"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .expect("write gerbil.pkg");
}
