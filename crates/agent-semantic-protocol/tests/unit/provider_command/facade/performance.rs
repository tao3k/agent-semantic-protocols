use std::time::{Duration, Instant};

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
};

const ASP_FACADE_PERFORMANCE_GATE: Duration = Duration::from_secs(1);

#[test]
fn language_facade_regular_commands_finish_inside_performance_gate() {
    let root = temp_project_root("language-facade-performance-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let providers = [
        ("rust", "rs-harness", "rs"),
        ("typescript", "ts-harness", "ts"),
        ("python", "py-harness", "py"),
    ];
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
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

    for (language, _, label) in providers.iter().copied() {
        let command_suite = [
            vec![language, "query", "Project.toml", "--query", "demo", "."],
            vec![language, "search", "prime", "--view", "seeds", "."],
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
            assert!(
                elapsed < ASP_FACADE_PERFORMANCE_GATE,
                "asp {args:?} exceeded {ASP_FACADE_PERFORMANCE_GATE:?}; elapsed={elapsed:?}; stdout={}; stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).expect("stdout");
            assert!(
                stdout.contains(&format!("{label} args=")),
                "args={args:?} stdout={stdout}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(root);
}
