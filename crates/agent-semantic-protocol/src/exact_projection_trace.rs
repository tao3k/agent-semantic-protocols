use std::time::Instant;

const TRACE_ENV: &str = "ASP_EXACT_QUERY_TRACE";

pub(crate) fn stage(stage: &str, started: Instant) {
    if enabled() {
        eprintln!(
            "[exact-query-trace] stage={stage} elapsedMicros={}",
            started.elapsed().as_micros()
        );
    }
}

fn enabled() -> bool {
    std::env::var_os(TRACE_ENV).is_some()
}
