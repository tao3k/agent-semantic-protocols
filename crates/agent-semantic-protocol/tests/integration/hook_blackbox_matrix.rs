use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MATRIX: &str =
    include_str!("../../../agent-semantic-config/templates/hooks/blackbox-test.toml");
const MATRIX_SCHEMA: &str =
    include_str!("../../../../schemas/semantic-hook-blackbox-test.v1.schema.json");

#[test]
fn process_environment_no_agent_bypasses_before_state_or_config_access() {
    let root = fixture_root();
    let state_home = root.join("state-home-that-does-not-exist");
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .args([
            "hook", "pre-tool", "--client", "codex", "--emit", "decision",
        ])
        .env("ASP_STATE_HOME", &state_home)
        .env("ASP_NO_AGENT", "1")
        .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn recovery-override Hook process");
    child
        .stdin
        .as_mut()
        .expect("Hook stdin")
        .write_all(
            json!({"tool_name":"Read", "tool_input":{"file_path":"src/lib.rs"}})
                .to_string()
                .as_bytes(),
        )
        .expect("write Host envelope");
    let output = child.wait_with_output().expect("wait for Hook decision");
    assert!(
        output.status.success(),
        "Hook recovery override failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).expect("decode Hook decision");
    assert_eq!(decision, json!({}), "{decision}");
    let stderr = String::from_utf8(output.stderr).expect("Hook trace UTF-8");
    let elapsed = stderr
        .lines()
        .find_map(|line| {
            line.strip_prefix("[asp-hook] route=bootstrap-asp-no-agent-passthrough elapsedMicros=")
        })
        .expect("bootstrap passthrough trace")
        .parse::<u64>()
        .expect("bootstrap passthrough elapsed micros");
    assert!(
        elapsed < 1_000,
        "ASP_NO_AGENT bootstrap path exceeded 1ms: elapsedMicros={elapsed} stderr={stderr}"
    );
    assert!(
        !state_home.exists(),
        "highest-priority bypass must not materialize Hook or Runtime state"
    );
    std::fs::remove_dir_all(root).expect("remove Hook recovery fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_matcher_generation_terminates_without_recursive_publication() {
    let root = fixture_root();
    let state_home = root.join(".agent-semantic-protocols");
    write_fixture(&root, &state_home);
    warm_executable_without_hook_state(&root, &state_home);
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .args([
            "hook", "pre-tool", "--client", "codex", "--emit", "decision",
        ])
        .env("ASP_STATE_HOME", &state_home)
        .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
        .env_remove("PRJ_CACHE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn unavailable Hook process");
    let mut stdin = child.stdin.take().expect("Hook stdin");
    stdin
        .write_all(
            json!({"tool_name":"Read", "tool_input":{"file_path":"src/lib.rs"}})
                .to_string()
                .as_bytes(),
        )
        .await
        .expect("write unavailable Hook payload");
    drop(stdin);
    let mut stdout = child.stdout.take().expect("Hook stdout");
    let mut stderr = child.stderr.take().expect("Hook stderr");
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let status = match tokio::time::timeout(std::time::Duration::from_secs(1), child.wait()).await {
        Ok(status) => status.expect("wait for unavailable Hook process"),
        Err(_) => {
            child
                .kill()
                .await
                .expect("kill deadlocked unavailable Hook process");
            let stdout = stdout_task
                .await
                .expect("join deadlocked Hook stdout")
                .expect("read deadlocked Hook stdout");
            let stderr = stderr_task
                .await
                .expect("join deadlocked Hook stderr")
                .expect("read deadlocked Hook stderr");
            panic!(
                "missing matcher generation caused a terminal deadlock: stdout={} stderr={}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
    };
    let stdout = stdout_task
        .await
        .expect("join Hook stdout")
        .expect("read Hook stdout");
    let stderr = stderr_task
        .await
        .expect("join Hook stderr")
        .expect("read Hook stderr");
    assert!(
        status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    let response: Value = serde_json::from_slice(&stdout).expect("decode unavailable Hook");
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("unavailable Hook context");
    assert!(context.contains("local-hook-policy-authority-unavailable"));
    assert!(context.contains("Binary v1 is not published"));
    assert!(
        !state_home.join("hooks/cache").exists(),
        "Hook data path attempted recursive matcher publication"
    );
    assert_server_absent(&state_home, "after missing generation");
    std::fs::remove_dir_all(root).expect("remove unavailable Hook fixture");
}

#[test]
fn config_derived_combinatorial_black_and_white_matrix_is_runtime_independent() {
    let matrix = toml::from_str::<toml::Value>(MATRIX).expect("parse Hook blackbox matrix");
    let matrix_json = serde_json::to_value(&matrix).expect("project Hook blackbox matrix");
    let schema = serde_json::from_str::<Value>(MATRIX_SCHEMA).expect("parse matrix schema");
    assert!(
        jsonschema::validator_for(&schema)
            .expect("compile matrix schema")
            .is_valid(&matrix_json),
        "Hook blackbox matrix is not schema-valid: {matrix_json}"
    );
    let root = fixture_root();
    let state_home = root.join(".agent-semantic-protocols");
    write_fixture(&root, &state_home);
    refresh_hook_matcher(&root, &state_home);
    let activation = crate::state_home_fixture::canonical_activation_path(&root, &state_home);

    let coverage = &matrix_json["coverage"];
    let max_wrapper_depth = coverage["maxWrapperDepth"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .expect("max wrapper depth");
    let include_negative_extension_mutation = coverage["includeNegativeExtensionMutation"]
        .as_bool()
        .expect("negative extension strategy");
    let config = toml::from_str(&agent_semantic_hook::default_client_config_template())
        .expect("parse rendered Hook client config");
    let warm_witnesses = agent_semantic_hook::policy_testing::combinatorial_policy_witnesses(
        &config,
        agent_semantic_hook::policy_testing::HookPolicyCombinatorialStrategy {
            max_wrapper_depth,
            include_negative_extension_mutation,
        },
    )
    .expect("generate config-derived warm witness");
    let warm_witness = warm_witnesses
        .iter()
        .find(|witness| {
            witness.polarity
                == agent_semantic_hook::policy_testing::HookPolicyWitnessPolarity::Black
                && witness.command_axis.is_none()
        })
        .expect("config-derived direct-read warm witness");
    let warm_path = root.join(&warm_witness.path);
    std::fs::create_dir_all(warm_path.parent().expect("warm witness parent"))
        .expect("create warm witness parent");
    std::fs::write(&warm_path, "generated Hook warm witness\n")
        .expect("write generated warm witness");
    // Compile and mmap the exact config-derived shard family before measuring.
    let warm = run_hook(
        &root,
        &state_home,
        &activation,
        json!({
            "tool_name": warm_witness.tool_name,
            "tool_input": warm_witness.tool_input.clone(),
        }),
    );
    assert_eq!(warm["decision"], "deny");
    let warm_again = run_hook(
        &root,
        &state_home,
        &activation,
        json!({
            "tool_name": warm_witness.tool_name,
            "tool_input": warm_witness.tool_input.clone(),
        }),
    );
    assert_eq!(warm_again["fields"]["hookMatcherGeneration"], "mmap-hit");
    assert_server_absent(&state_home, "after warmup");

    let max_decision_micros = matrix_json["maxDecisionMicros"]
        .as_u64()
        .expect("matrix decision budget");
    assert_config_derived_policy_combinations(
        &root,
        &state_home,
        &activation,
        max_decision_micros,
        matrix_json["typicalDecisionMicros"]
            .as_u64()
            .expect("typical decision target"),
        max_wrapper_depth,
        include_negative_extension_mutation,
        coverage["minimumWitnesses"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .expect("minimum generated witness count"),
    );

    std::fs::remove_dir_all(root).expect("remove Hook blackbox fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_hook_processes_are_lock_free_and_each_stays_below_one_millisecond() {
    const CONCURRENCY: usize = 32;
    const TYPICAL_QUORUM: usize = CONCURRENCY * 3 / 4;
    let matrix = toml::from_str::<toml::Value>(MATRIX).expect("parse Hook blackbox matrix");
    let typical_decision_micros = matrix["typicalDecisionMicros"]
        .as_integer()
        .and_then(|value| u64::try_from(value).ok())
        .expect("matrix typical decision target");
    let root = fixture_root();
    let state_home = root.join(".agent-semantic-protocols");
    write_fixture(&root, &state_home);
    refresh_hook_matcher(&root, &state_home);
    let activation = crate::state_home_fixture::canonical_activation_path(&root, &state_home);
    let warm = run_hook(
        &root,
        &state_home,
        &activation,
        json!({"tool_name":"Read", "tool_input":{"file_path":"Cargo.lock"}}),
    );
    assert_eq!(warm["decision"], "allow");

    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..CONCURRENCY {
        let root = root.clone();
        let state_home = state_home.clone();
        let activation = activation.clone();
        tasks.spawn(async move {
            let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_asp"))
                .current_dir(&root)
                .args([
                    "hook",
                    "pre-tool",
                    "--client",
                    "codex",
                    "--emit",
                    "decision",
                    "--activation",
                ])
                .arg(&activation)
                .env("ASP_STATE_HOME", &state_home)
                .env_remove("PRJ_CACHE_HOME")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .expect("spawn concurrent Hook process");
            let payload = json!({
                "tool_name":"Read",
                "tool_input":{"file_path": format!("src/concurrent-{index}.rs")}
            });
            let mut stdin = child.stdin.take().expect("Hook stdin");
            stdin
                .write_all(payload.to_string().as_bytes())
                .await
                .expect("write concurrent Host envelope");
            drop(stdin);
            let mut stdout = child.stdout.take().expect("Hook stdout");
            let stdout_task = tokio::spawn(async move {
                let mut bytes = Vec::new();
                stdout.read_to_end(&mut bytes).await.map(|_| bytes)
            });
            let status = tokio::time::timeout(std::time::Duration::from_secs(1), child.wait())
                .await
                .expect("concurrent Hook process exceeded one-second terminal safety boundary")
                .expect("wait for concurrent Hook process");
            assert!(status.success(), "index={index}");
            let stdout = stdout_task
                .await
                .expect("join concurrent Hook stdout")
                .expect("read concurrent Hook stdout");
            let decision: Value =
                serde_json::from_slice(&stdout).expect("decode concurrent Hook decision");
            (index, decision)
        });
    }
    let mut elapsed_samples = Vec::with_capacity(CONCURRENCY);
    while let Some(result) = tasks.join_next().await {
        let (index, decision) = result.expect("join concurrent Hook process");
        assert_eq!(decision["decision"], "deny", "index={index}: {decision}");
        assert_eq!(
            decision["fields"]["hookDecisionBudgetStatus"], "within-budget",
            "index={index}: {decision}"
        );
        let cpu = decision["fields"]["hookDecisionCpuMicros"]
            .as_u64()
            .unwrap_or_else(|| panic!("index={index}: {decision}"));
        assert!(cpu < 1_000, "index={index}: {decision}");
        let elapsed = decision["fields"]["hookDecisionElapsedMicros"]
            .as_u64()
            .unwrap_or_else(|| panic!("index={index}: {decision}"));
        elapsed_samples.push(elapsed);
    }
    elapsed_samples.sort_unstable();
    eprintln!(
        "[hook-concurrency-latency] samples={} p50Micros={} p75Micros={} maxMicros={} targetMicros={} boundaryMicros=1000",
        elapsed_samples.len(),
        elapsed_samples[CONCURRENCY / 2],
        elapsed_samples[TYPICAL_QUORUM - 1],
        elapsed_samples[CONCURRENCY - 1],
        typical_decision_micros,
    );
    assert!(
        elapsed_samples[TYPICAL_QUORUM - 1] <= typical_decision_micros,
        "fewer than {TYPICAL_QUORUM}/{CONCURRENCY} concurrent decisions met the typical \
         {typical_decision_micros}us target: {elapsed_samples:?}"
    );
    assert_server_absent(&state_home, "after concurrent Hook pressure");
    std::fs::remove_dir_all(root).expect("remove concurrent Hook fixture");
}

fn assert_config_derived_policy_combinations(
    root: &Path,
    state_home: &Path,
    activation: &Path,
    max_decision_micros: u64,
    typical_decision_micros: u64,
    max_wrapper_depth: usize,
    include_negative_extension_mutation: bool,
    minimum_witnesses: usize,
) {
    let config = toml::from_str(&agent_semantic_hook::default_client_config_template())
        .expect("parse rendered Hook client config");
    let witnesses = agent_semantic_hook::policy_testing::combinatorial_policy_witnesses(
        &config,
        agent_semantic_hook::policy_testing::HookPolicyCombinatorialStrategy {
            max_wrapper_depth,
            include_negative_extension_mutation,
        },
    )
    .expect("generate config-derived combinatorial Hook witnesses");
    assert!(witnesses.len() >= minimum_witnesses, "{}", witnesses.len());
    let mut covered_extensions = std::collections::BTreeSet::new();
    let mut covered_envelopes = std::collections::BTreeSet::new();
    let mut covered_commands = std::collections::BTreeSet::new();
    let mut covered_wrapper_depths = std::collections::BTreeSet::new();
    let mut elapsed_samples = Vec::with_capacity(witnesses.len());
    for witness in &witnesses {
        let absolute = root.join(&witness.path);
        std::fs::create_dir_all(absolute.parent().expect("generated witness parent"))
            .expect("create generated witness parent");
        std::fs::write(&absolute, "generated Hook provider witness\n")
            .expect("write generated provider witness");
    }
    let payloads = witnesses
        .iter()
        .map(|witness| {
            json!({"tool_name": witness.tool_name, "tool_input": witness.tool_input.clone()})
        })
        .collect::<Vec<_>>();
    let decisions = run_hook_matrix_parallel(root, state_home, activation, &payloads);
    for (witness, decision) in witnesses.iter().zip(&decisions) {
        covered_extensions.insert(witness.source_extension.clone());
        covered_envelopes.insert(witness.envelope_axis.clone());
        covered_commands.extend(witness.command_axis.iter().cloned());
        covered_wrapper_depths.insert(witness.wrapper_depth);
        assert_eq!(
            decision["reasonKind"], witness.expected_reason_kind,
            "{}: {decision}",
            witness.id
        );
        assert_eq!(
            decision["languageIds"],
            serde_json::to_value(&witness.expected_language_ids).expect("expected language ids"),
            "{}: {decision}",
            witness.id
        );
        let actual_routes = decision["routes"]
            .as_array()
            .expect("decision routes")
            .iter()
            .filter_map(|route| {
                Some((
                    route["providerId"].as_str()?.to_owned(),
                    route["languageId"].as_str()?.to_owned(),
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_routes, witness.expected_routes, "{}", witness.id);
        assert_eq!(
            decision["fields"]["configRuleId"].as_str(),
            witness.expected_rule_id.as_deref(),
            "{}: {decision}",
            witness.id
        );
        let expected_decision = match witness.expected_decision {
            agent_semantic_hook::DecisionKind::Allow => "allow",
            agent_semantic_hook::DecisionKind::Block => "block",
            agent_semantic_hook::DecisionKind::Deny => "deny",
        };
        assert_eq!(
            decision["decision"], expected_decision,
            "{}: {decision}",
            witness.id
        );
        assert_eq!(
            decision["fields"]["hookMatcherGeneration"], "mmap-hit",
            "{}",
            witness.id
        );
        assert_eq!(
            decision["fields"]["hookMatcherProjection"],
            if witness.command_axis.is_some() {
                "shell-read-decision-shard"
            } else {
                "direct-read-decision-shard"
            },
            "{}: {decision}",
            witness.id
        );
        assert_eq!(
            decision["fields"]["hookPolicySynchronousDependencies"],
            json!([]),
            "{}",
            witness.id
        );
        elapsed_samples.push((
            decision["fields"]["hookDecisionElapsedMicros"]
                .as_u64()
                .expect("Hook decision elapsed micros"),
            decision["fields"]["hookDecisionCpuMicros"]
                .as_u64()
                .expect("Hook decision thread CPU micros"),
            witness.id.clone(),
        ));
        assert_server_absent(state_home, &witness.id);
    }
    let configured_extensions = config
        .language_providers
        .iter()
        .flat_map(|provider| provider.source_extensions.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(covered_extensions, configured_extensions);
    assert!(covered_envelopes.len() >= 8, "{covered_envelopes:?}");
    assert!(covered_commands.len() >= 6, "{covered_commands:?}");
    assert_eq!(covered_wrapper_depths.len(), max_wrapper_depth + 1);
    assert!(witnesses.iter().any(|witness| {
        witness.polarity == agent_semantic_hook::policy_testing::HookPolicyWitnessPolarity::Black
    }));
    assert!(witnesses.iter().any(|witness| {
        witness.polarity == agent_semantic_hook::policy_testing::HookPolicyWitnessPolarity::White
    }));
    if !cfg!(debug_assertions) {
        elapsed_samples.sort_unstable_by_key(|(elapsed, _, _)| *elapsed);
        let p50 = elapsed_samples[elapsed_samples.len() / 2].0;
        let p75 = elapsed_samples[elapsed_samples.len() * 3 / 4].0;
        let (wall_max, _, wall_max_id) =
            elapsed_samples.last().expect("Hook matrix latency samples");
        let (cpu_max, cpu_max_id) = elapsed_samples
            .iter()
            .map(|(_, cpu, id)| (*cpu, id))
            .max_by_key(|(cpu, _)| *cpu)
            .expect("Hook matrix CPU samples");
        eprintln!(
            "[hook-combinatorial-latency] samples={} wallP50Micros={p50} wallP75Micros={p75} wallMaxMicros={wall_max} wallMaxCase={wall_max_id} cpuMaxMicros={cpu_max} cpuMaxCase={cpu_max_id}",
            elapsed_samples.len()
        );
        assert!(
            cpu_max < max_decision_micros,
            "config-derived matrix exceeded {max_decision_micros}us CPU: max={cpu_max} case={cpu_max_id}"
        );
        assert!(
            p75 <= typical_decision_micros,
            "fewer than 75% of config-derived decisions met the typical \
             {typical_decision_micros}us target: p75={p75}"
        );
    }
}

fn run_hook(root: &Path, state_home: &Path, activation: &Path, payload: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .args([
            "hook",
            "pre-tool",
            "--client",
            "codex",
            "--emit",
            "decision",
            "--activation",
        ])
        .arg(activation)
        .env("ASP_STATE_HOME", state_home)
        .env_remove("PRJ_CACHE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Hook blackbox process");
    child
        .stdin
        .as_mut()
        .expect("Hook stdin")
        .write_all(payload.to_string().as_bytes())
        .expect("write Host envelope");
    let output = child.wait_with_output().expect("wait for Hook decision");
    assert!(
        output.status.success(),
        "Hook blackbox process failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decode Hook decision")
}

fn run_hook_matrix_parallel(
    root: &Path,
    state_home: &Path,
    activation: &Path,
    payloads: &[Value],
) -> Vec<Value> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let next = AtomicUsize::new(0);
    let results = Mutex::new(vec![None; payloads.len()]);
    let worker_count = 8.min(payloads.len());
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(payload) = payloads.get(index) else {
                        break;
                    };
                    let decision = run_hook(root, state_home, activation, payload.clone());
                    results.lock().expect("Hook matrix result lock")[index] = Some(decision);
                }
            });
        }
    });
    results
        .into_inner()
        .expect("Hook matrix result lock")
        .into_iter()
        .enumerate()
        .map(|(index, decision)| {
            decision.unwrap_or_else(|| panic!("Hook matrix worker omitted witness {index}"))
        })
        .collect()
}

fn refresh_hook_matcher(root: &Path, state_home: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .args(["hook", "refresh", "--client", "codex"])
        .env("ASP_STATE_HOME", state_home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run Hook matcher control-plane refresh");
    assert!(
        output.status.success(),
        "Hook matcher refresh failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = String::from_utf8(output.stdout).expect("Hook refresh receipt UTF-8");
    assert!(receipt.contains("binarySchemaVersion=1"), "{receipt}");
    assert!(receipt.contains("mode=atomically-published"), "{receipt}");
}

fn warm_executable_without_hook_state(root: &Path, state_home: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .arg("--help")
        .env("ASP_STATE_HOME", state_home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("warm debug Hook executable pages");
    assert!(
        output.status.success(),
        "Hook executable warmup failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !state_home.join("hooks/cache").exists(),
        "executable warmup must not publish Hook matcher state"
    );
}

fn write_fixture(root: &Path, state_home: &Path) {
    let git = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .expect("initialize Hook fixture Git workspace");
    assert!(git.success(), "initialize Hook fixture Git workspace");
    for (path, content) in [
        ("src/lib.rs", "pub fn blackbox() {}\n"),
        ("src/main.py", "def blackbox():\n    pass\n"),
        ("README.md", "# Hook blackbox\n"),
        ("DESIGN.org", "* Hook blackbox\n"),
        ("Cargo.lock", "# unregistered lockfile witness\n"),
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create fixture parent");
        std::fs::write(path, content).expect("write Hook blackbox owner");
    }
    for language in [
        "rust",
        "typescript",
        "python",
        "julia",
        "gerbil-scheme",
        "md",
        "org",
    ] {
        crate::state_home_fixture::install_provider_script(
            state_home,
            language,
            "#!/bin/sh\nexit 0\n",
        );
    }
    crate::state_home_fixture::write_activation(
        root,
        state_home,
        &[
            "rust",
            "typescript",
            "python",
            "julia",
            "gerbil-scheme",
            "md",
            "org",
        ],
    );
    std::fs::create_dir_all(root.join(".agent-semantic-protocols/hooks"))
        .expect("create Hook config owner");
    std::fs::write(
        root.join(".agent-semantic-protocols/hooks/config.toml"),
        agent_semantic_hook::default_client_config_template(),
    )
    .expect("write Hook config");
}

fn assert_server_absent(state_home: &Path, stage: &str) {
    let server = state_home.join("runtime/server");
    for artifact in ["endpoint.v1.json", "run-intent.v1", "owner-spawn.v1.json"] {
        assert!(
            !server.join(artifact).exists(),
            "Hook created Runtime Server artifact {artifact} at {stage}"
        );
    }
}

fn fixture_root() -> PathBuf {
    tempfile::Builder::new()
        .prefix("asp-hook-blackbox-")
        .tempdir()
        .expect("Hook blackbox tempdir")
        .keep()
}
