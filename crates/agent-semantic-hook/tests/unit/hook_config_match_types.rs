// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::CompiledCommandContains;
use super::CompiledPathGlobs;

#[cfg(unix)]
fn current_thread_cpu_nanos() -> u128 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `time` is a valid writable timespec and the clock is process-local.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    assert_eq!(status, 0, "read current-thread CPU clock");
    (time.tv_sec as u128) * 1_000_000_000 + (time.tv_nsec as u128)
}

#[cfg(not(unix))]
fn current_thread_cpu_nanos() -> u128 {
    static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    STARTED
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos()
}

#[test]
fn durable_matchers_preserve_live_command_and_path_semantics() {
    let live_command = crate::hook_config::core::compile::compile_command_contains(vec![
        "Cargo Check".to_owned(),
        "src/Exact.rs".to_owned(),
    ])
    .expect("compile live command matcher");
    let durable_command = CompiledCommandContains::from_durable(live_command.durable_artifact())
        .expect("hydrate command matcher");
    for command in [
        "direnv exec . cargo check -p crate",
        "cat SRC/exact.RS",
        "cargo test",
        "echo unrelated",
    ] {
        assert_eq!(
            durable_command.matches(command),
            live_command.matches(command),
            "command matcher drift for {command:?}"
        );
    }

    let live_paths = crate::hook_config::core::compile::compile_globs(
        "testPathGlobAny",
        vec![
            "**/*.rs".to_owned(),
            "*.rs".to_owned(),
            "crates/**/tests/*.toml".to_owned(),
        ],
    )
    .expect("compile live path matcher");
    let durable_paths = CompiledPathGlobs::from_durable(live_paths.durable_artifact())
        .expect("hydrate path matcher");
    for path in [
        "src/lib.rs",
        "lib.rs",
        "crates/hook/tests/config.toml",
        "crates/hook/src/config.toml",
        "README.md",
    ] {
        assert_eq!(
            durable_paths.matches(path),
            live_paths.matches(path),
            "path matcher drift for {path:?}"
        );
    }
}

#[test]
fn durable_matcher_hydrate_and_match_p99_is_sub_millisecond() {
    let live = crate::hook_config::core::compile::compile_globs(
        "performancePathGlobAny",
        vec![
            "**/*.rs".to_owned(),
            "*.rs".to_owned(),
            "crates/**/tests/*.toml".to_owned(),
        ],
    )
    .expect("compile live path matcher");
    let artifact = live.durable_artifact();
    let mut samples = (0..10_000)
        .map(|_| {
            let started = current_thread_cpu_nanos();
            let matcher = CompiledPathGlobs::from_durable(artifact.clone())
                .expect("hydrate durable path matcher");
            std::hint::black_box(matcher.matches("crates/hook/tests/config.toml"));
            current_thread_cpu_nanos() - started
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[hook-durable-matcher] requests=10000 p99Nanos={} builderInvocations=0",
        p99
    );
    assert!(
        p99 < 1_000_000,
        "durable matcher hydrate+match p99 must remain sub-millisecond of thread CPU, observed {p99}ns"
    );
}
