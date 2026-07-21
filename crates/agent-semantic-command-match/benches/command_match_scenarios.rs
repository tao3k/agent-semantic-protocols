use agent_semantic_command_match::{command_stage_matches_prefix, PrefixMatch, MAX_PREFIX_WINDOWS};
use std::hint::black_box;
use std::time::Instant;

const SAMPLES: usize = 64;
const ITERATIONS_PER_SAMPLE: usize = 2_048;
const RULE_SET_ITERATIONS_PER_SAMPLE: usize = 64;
const SINGLE_MATCH_P95_BUDGET_NS: u128 = 50_000;
const RULE_SET_P95_BUDGET_NS: u128 = 1_000_000;

struct Scenario {
    name: &'static str,
    command: Vec<String>,
    expected: PrefixMatch,
}

fn tokens(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn measure_p95_ns_per_op(iterations: usize, mut operation: impl FnMut()) -> u128 {
    for _ in 0..iterations {
        operation();
    }

    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        samples.push(started.elapsed().as_nanos() / iterations as u128);
    }
    samples.sort_unstable();
    samples[(SAMPLES * 95 / 100).min(SAMPLES - 1)]
}

fn main() {
    let prefix = tokens(&["cargo", "test"]);
    let mut worst_case = (0..MAX_PREFIX_WINDOWS)
        .map(|index| format!("wrapper-{index}"))
        .collect::<Vec<_>>();
    worst_case.extend(prefix.clone());

    let scenarios = [
        Scenario {
            name: "bare",
            command: tokens(&["cargo", "test", "-p", "policy"]),
            expected: PrefixMatch::Matched,
        },
        Scenario {
            name: "absolute-executable",
            command: tokens(&["/Users/example/.cargo/bin/cargo", "test", "--workspace"]),
            expected: PrefixMatch::Matched,
        },
        Scenario {
            name: "env-wrapper",
            command: tokens(&["env", "RUST_BACKTRACE=1", "cargo", "test"]),
            expected: PrefixMatch::Matched,
        },
        Scenario {
            name: "direnv-wrapper",
            command: tokens(&["direnv", "exec", ".", "cargo", "test"]),
            expected: PrefixMatch::Matched,
        },
        Scenario {
            name: "rtk-wrapper",
            command: tokens(&["rtk", "test", "/opt/rust/bin/cargo", "test"]),
            expected: PrefixMatch::Matched,
        },
        Scenario {
            name: "shell-stage",
            command: tokens(&["echo", "ready", "&&", "cargo", "test"]),
            expected: PrefixMatch::Matched,
        },
        Scenario {
            name: "quoted-negative",
            command: tokens(&["echo", "cargo test"]),
            expected: PrefixMatch::NotMatched,
        },
        Scenario {
            name: "bounded-worst-case",
            command: worst_case,
            expected: PrefixMatch::BudgetExceeded,
        },
    ];

    for scenario in scenarios {
        let actual = command_stage_matches_prefix(&scenario.command, &prefix);
        assert_eq!(actual, scenario.expected, "scenario={}", scenario.name);
        let p95_ns = measure_p95_ns_per_op(ITERATIONS_PER_SAMPLE, || {
            black_box(command_stage_matches_prefix(
                black_box(&scenario.command),
                black_box(&prefix),
            ));
        });
        println!(
            "[match-bench] scenario={} p95Ns={} budgetNs={} state=ok",
            scenario.name, p95_ns, SINGLE_MATCH_P95_BUDGET_NS
        );
        assert!(
            p95_ns <= SINGLE_MATCH_P95_BUDGET_NS,
            "scenario={} p95Ns={} exceeds budgetNs={}",
            scenario.name,
            p95_ns,
            SINGLE_MATCH_P95_BUDGET_NS
        );
    }

    let rule_prefixes = (0..127)
        .map(|index| tokens(&[&format!("tool-{index}"), "run"]))
        .chain(std::iter::once(prefix.clone()))
        .collect::<Vec<_>>();
    let routed_command = tokens(&["direnv", "exec", ".", "/opt/rust/bin/cargo", "test"]);
    let p95_ns = measure_p95_ns_per_op(RULE_SET_ITERATIONS_PER_SAMPLE, || {
        let matched = black_box(&rule_prefixes).iter().any(|rule_prefix| {
            command_stage_matches_prefix(black_box(&routed_command), black_box(rule_prefix))
                .routes_protected()
        });
        assert!(matched);
    });
    println!(
        "[match-bench] scenario=128-rule-route p95Ns={} budgetNs={} state=ok",
        p95_ns, RULE_SET_P95_BUDGET_NS
    );
    assert!(
        p95_ns <= RULE_SET_P95_BUDGET_NS,
        "scenario=128-rule-route p95Ns={} exceeds budgetNs={}",
        p95_ns,
        RULE_SET_P95_BUDGET_NS
    );
}
