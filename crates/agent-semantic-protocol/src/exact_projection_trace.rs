const TRACE_ENV: &str = "ASP_EXACT_QUERY_TRACE";

pub(crate) fn stage(stage: &str, started: tokio::time::Instant) {
    if enabled() {
        eprintln!(
            "[exact-query-trace] stage={stage} elapsedMicros={}",
            started.elapsed().as_micros()
        );
    }
}

pub(crate) fn generation(stage: &str, generation_digest: &str) {
    if enabled() {
        eprintln!("[exact-query-trace] stage={stage} generationDigest={generation_digest}");
    }
}

fn enabled() -> bool {
    std::env::var_os(TRACE_ENV).is_some()
}
