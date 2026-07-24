use std::{hint::black_box, time::Instant};

#[path = "../tests/support/match_config.rs"]
mod match_config;

const ITERATIONS: usize = 10_000;
const P95_BUDGET_NS: u128 = 1_000_000;

fn main() {
    assert!(match_config::wrapper_match_enabled());
    let cases = match_config::rule_prefixes();
    for case in &cases {
        match_config::assert_case(case);
    }
    let commands = cases
        .iter()
        .flat_map(|case| {
            match_config::positive_commands(case)
                .into_iter()
                .chain(match_config::negative_commands(case))
                .chain(match_config::invalid_commands(case))
                .map(move |command| (case, command))
        })
        .collect::<Vec<_>>();

    let mut samples = Vec::with_capacity(ITERATIONS);
    for iteration in 0..ITERATIONS {
        let (case, command) = &commands[iteration % commands.len()];
        let started = Instant::now();
        black_box(match_config::outcome(black_box(case), black_box(command)));
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();

    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95) / 100];
    let max = samples[samples.len() - 1];
    println!(
        "[command-match-bench] rules={} commands={} iterations={} medianNs={} p95Ns={} maxNs={} budgetNs={}",
        cases.len(),
        commands.len(),
        ITERATIONS,
        median,
        p95,
        max,
        P95_BUDGET_NS
    );
    assert!(
        p95 < P95_BUDGET_NS,
        "command-match p95={p95}ns exceeds budget={P95_BUDGET_NS}ns"
    );
}
