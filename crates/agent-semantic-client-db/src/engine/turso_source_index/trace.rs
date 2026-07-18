pub(super) fn source_index_db_trace(stage: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage={stage} elapsedMs={}",
            started.elapsed().as_millis()
        );
    }
}

pub(super) fn source_index_db_trace_row_counts(
    stage: &str,
    started: std::time::Instant,
    owner_count: usize,
    term_count: usize,
) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage={stage} elapsedMs={} owners={owner_count} terms={term_count}",
            started.elapsed().as_millis()
        );
    }
}

pub(super) fn source_index_db_trace_membership_changes(
    started: std::time::Instant,
    changed_owner_count: usize,
    removed_owner_count: usize,
) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage=snapshot-membership-joined elapsedMs={} changedOwners={changed_owner_count} removedOwners={removed_owner_count}",
            started.elapsed().as_millis()
        );
    }
}

pub(super) fn source_index_db_trace_posting_projection(
    started: std::time::Instant,
    posting_count: usize,
) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage=snapshot-posting-projection-written elapsedMs={} postings={posting_count}",
            started.elapsed().as_millis(),
        );
    }
}
