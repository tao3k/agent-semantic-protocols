use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_check_failure_provider, write_stdout_stderr_exit_provider,
};
use std::time::{Duration, Instant};

const GERBIL_CHECK_CACHE_REPLAY_WALL_SANITY_GATE: Duration = Duration::from_secs(2);
const FNV64_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV64_PRIME: u64 = 1_099_511_628_211;

#[test]
fn check_changed_view_seeds_renders_failure_frontier_after_provider_failure() {
    let root = temp_project_root("provider-check-failure-frontier-facade");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/cache_cli")).expect("create src dir");
    std::fs::write(
        root.join("src/cache_cli/writeback.rs"),
        "pub fn write_prompt_output_artifact() {\n    let request_fingerprint = \"miss\";\n    let file_hash = request_fingerprint;\n    assert!(!file_hash.is_empty());\n}\n",
    )
    .expect("write source");
    write_check_failure_provider(
        &bin_dir,
        "rs-harness",
        "cache_cli::write_prompt_output_artifact expected hit actual miss\nrequest_fingerprint file_hash\n",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rust", "check", "changed", "--view", "seeds", "."])
        .output()
        .expect("run asp rust check changed seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.starts_with(
            "[search-failure] kind=test-failure profile=failure-frontier alg=typed-ppr-diverse"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("F=failure:test-failure(")
            && stdout.contains("write_prompt_output_artifact"),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontierActions=")
            && stdout.contains("C1.query-code(selector=src/cache_cli/writeback.rs:1:5"),
        "{stdout}"
    );
    assert!(
        stdout.contains("queryProfiles=failure-frontier(F=>failure-facts+owners+hot-blocks)"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "entries=failure-frontier(F=>failure-facts+candidate-owners+hot-blocks+query-profiles)"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("omit=full-source,unrelated-functions,wide-windows"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=manual-window-scan,duplicate-read,raw-read,broad-fzf"),
        "{stdout}"
    );
    for debug_prefix in [
        "scores=", "paths=", "cache=", "trace=", "explain=", "metrics=",
    ] {
        assert!(
            !stdout.lines().any(|line| line.starts_with(debug_prefix)),
            "{stdout}"
        );
    }
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(!stderr.contains("unexpected --view"), "{stderr}");
    let last_check =
        std::fs::read_to_string(root.join(".cache/agent-semantic-protocol/last-check-output.txt"))
            .expect("last check output");
    assert!(
        last_check.contains("cache_cli::write_prompt_output_artifact"),
        "{last_check}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_check_full_replays_valid_output_cache_without_provider_spawn() {
    let root = temp_project_root("gerbil-check-full-cache-replay");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let source_path = root.join("src/main.ss");
    std::fs::create_dir_all(source_path.parent().expect("source parent")).expect("create src");
    std::fs::write(&source_path, "(def (cached-check) 'ok)\n").expect("write source");
    write_stdout_stderr_exit_provider(
        &bin_dir,
        "gslph",
        "provider should not run\n",
        "provider should not run\n",
        66,
    );
    write_activation(
        &root,
        &[provider(
            "gerbil-scheme",
            vec![bin_dir.join("gslph").display().to_string()],
        )],
    );
    write_gerbil_check_text_cache(
        &root,
        &[source_path.display().to_string()],
        &[],
        "[gerbil-check] status=fail scope=full files=1 definitions=1 findings=1\n",
    );

    let started_at = Instant::now();
    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args(["gerbil-scheme", "check", "--full", "."])
        .output()
        .expect("run asp gerbil check");
    let elapsed = started_at.elapsed();

    assert!(
        output.status.success(),
        "status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("findings=1") && !stdout.contains("provider should not run"),
        "{stdout}"
    );
    assert!(
        elapsed < GERBIL_CHECK_CACHE_REPLAY_WALL_SANITY_GATE,
        "Gerbil check cache replay exceeded {GERBIL_CHECK_CACHE_REPLAY_WALL_SANITY_GATE:?}; elapsed={elapsed:?}; stdout={stdout}; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_check_full_replays_workspace_output_cache_without_provider_spawn() {
    let root = temp_project_root("gerbil-check-full-workspace-cache-replay");
    let workspace = root.join("language");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let source_path = workspace.join("src/main.ss");
    std::fs::create_dir_all(source_path.parent().expect("source parent")).expect("create src");
    std::fs::write(&source_path, "(def (cached-workspace-check) 'ok)\n").expect("write source");
    write_stdout_stderr_exit_provider(
        &bin_dir,
        "gslph",
        "provider should not run\n",
        "provider should not run\n",
        66,
    );
    write_activation(
        &root,
        &[provider(
            "gerbil-scheme",
            vec![bin_dir.join("gslph").display().to_string()],
        )],
    );
    write_gerbil_check_text_cache(
        &workspace,
        &[source_path.display().to_string()],
        &[],
        "[gerbil-check] status=fail scope=full files=1 definitions=1 findings=1\n",
    );

    let started_at = Instant::now();
    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "gerbil-scheme",
            "check",
            "--workspace",
            "language",
            "--full",
        ])
        .output()
        .expect("run asp gerbil workspace check");
    let elapsed = started_at.elapsed();

    assert!(
        output.status.success(),
        "status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("findings=1") && !stdout.contains("provider should not run"),
        "{stdout}"
    );
    assert!(
        elapsed < GERBIL_CHECK_CACHE_REPLAY_WALL_SANITY_GATE,
        "Gerbil workspace check cache replay exceeded {GERBIL_CHECK_CACHE_REPLAY_WALL_SANITY_GATE:?}; elapsed={elapsed:?}; stdout={stdout}; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

fn write_gerbil_check_text_cache(
    root: &std::path::Path,
    inputs: &[String],
    directories: &[String],
    output: &str,
) {
    let cache_dir = root.join(".cache/agent-semantic-protocol/gerbil-scheme/check");
    std::fs::create_dir_all(&cache_dir).expect("create check cache dir");
    let fingerprint = gerbil_check_fingerprint(root, inputs, directories);
    let input_list = inputs
        .iter()
        .map(|path| scheme_string(path))
        .collect::<Vec<_>>()
        .join(" ");
    let directory_list = directories
        .iter()
        .map(|path| scheme_string(path))
        .collect::<Vec<_>>()
        .join(" ");
    let cache = format!(
        "((version . {version}) (fingerprint . {fingerprint}) (inputs {input_list}) (directories {directory_list}) (status . 1) (output . {output}))",
        version = scheme_string("check-full-output-cache.v1"),
        fingerprint = scheme_string(&fingerprint),
        output = scheme_string(output)
    );
    std::fs::write(cache_dir.join("text.sexp"), cache).expect("write check cache");
}

fn gerbil_check_fingerprint(
    root: &std::path::Path,
    inputs: &[String],
    directories: &[String],
) -> String {
    format!(
        "(version: {} mode: {} inputs: ({}) directories: ({}))",
        scheme_string("check-full-output-cache.v1"),
        scheme_string("source-inputs"),
        inputs
            .iter()
            .map(|path| gerbil_file_fingerprint(root, path))
            .collect::<Vec<_>>()
            .join(" "),
        directories
            .iter()
            .map(|path| gerbil_file_fingerprint(root, path))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn gerbil_file_fingerprint(root: &std::path::Path, path: &str) -> String {
    let path_buf = std::path::PathBuf::from(path);
    let expanded = if path_buf.is_absolute() {
        path_buf
    } else {
        root.join(path_buf)
    };
    let metadata = std::fs::metadata(&expanded).expect("fingerprint metadata");
    if metadata.is_dir() {
        let entries = sorted_directory_entries(&expanded);
        return format!(
            "({} directory ({}))",
            scheme_string(path),
            entries
                .iter()
                .map(|entry| scheme_string(entry))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    format!(
        "({} file {} {})",
        scheme_string(path),
        metadata.len(),
        fnv64_file_hash(&expanded)
    )
}

fn sorted_directory_entries(path: &std::path::Path) -> Vec<String> {
    let mut entries = std::fs::read_dir(path)
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn fnv64_file_hash(path: &std::path::Path) -> u64 {
    std::fs::read(path)
        .expect("read fingerprint file")
        .into_iter()
        .fold(FNV64_OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV64_PRIME)
        })
}

fn scheme_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}
