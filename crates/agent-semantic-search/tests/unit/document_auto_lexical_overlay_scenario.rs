use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use orgize::document::DocumentLanguage;

use crate::{
    SearchPipeDocumentAcquisitionRequest, SearchPipeSourceMode,
    collect_search_pipe_document_acquisition,
};

const SCENARIO_ID: &str = "document-auto-lexical-overlay-warm-path";
const SCENARIO_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/unit/scenarios/document_auto_lexical_overlay_warm_path"
);

#[test]
fn document_auto_lexical_overlay_warm_path_stays_inside_scenario_gate() {
    let scenario = fs::read_to_string(Path::new(SCENARIO_ROOT).join("scenario.toml"))
        .expect("read scenario manifest");
    let benchmark = fs::read_to_string(Path::new(SCENARIO_ROOT).join("benchmark.toml"))
        .expect("read benchmark manifest");
    assert!(scenario.contains(&format!("id = \"{SCENARIO_ID}\"")));
    assert!(
        scenario.contains("SEARCH-AGENT-ASP-PERF-SUBCOMMAND-DOCUMENT-AUTO-LEXICAL-OVERLAY-001")
    );
    assert_benchmark_contract(&benchmark);
    let max_total = duration_from_manifest(&benchmark, "max_total");

    let root = tempfile::tempdir().expect("create scenario root");
    fs::create_dir_all(root.path().join("docs")).expect("create docs");
    fs::write(
        root.path().join("docs").join("plan.org"),
        "* Plan\n\nThe document_auto_overlay_fixture token lives in Org body text.\n",
    )
    .expect("write org fixture");

    let ignore_dirs = vec!["target".to_string(), "node_modules".to_string()];
    let include_hidden_dirs = Vec::new();

    let started_at = Instant::now();
    let acquisition =
        collect_search_pipe_document_acquisition(SearchPipeDocumentAcquisitionRequest {
            language: DocumentLanguage::Org,
            project_root: root.path(),
            locator_root: root.path(),
            intent: "document_auto_overlay_fixture",
            scopes: &[],
            mode: SearchPipeSourceMode::Auto,
            ignore_dirs: &ignore_dirs,
            include_hidden_dirs: &include_hidden_dirs,
            search_overlay_limit: 16,
        })
        .expect("collect auto document candidates");
    let elapsed = started_at.elapsed();

    assert!(
        elapsed <= max_total,
        "document auto lexical overlay exceeded max_total={max_total:?} observed={elapsed:?}"
    );
    assert_eq!(
        acquisition.candidate_sources,
        vec!["search-overlay".to_string()]
    );
    assert_eq!(acquisition.source_trace.len(), 1);
    assert_eq!(acquisition.source_trace[0].source, "search-overlay");
    assert_eq!(acquisition.source_trace[0].status, "used");
    assert!(
        acquisition.candidates.iter().any(|candidate| {
            candidate.path == "docs/plan.org"
                && candidate.source == "search-overlay"
                && candidate.confidence == "lexical-overlay"
        }),
        "candidates={:?}",
        acquisition.candidates
    );

    let provider_acquisition =
        collect_search_pipe_document_acquisition(SearchPipeDocumentAcquisitionRequest {
            language: DocumentLanguage::Org,
            project_root: root.path(),
            locator_root: root.path(),
            intent: "document_auto_overlay_fixture",
            scopes: &[],
            mode: SearchPipeSourceMode::Provider,
            ignore_dirs: &ignore_dirs,
            include_hidden_dirs: &include_hidden_dirs,
            search_overlay_limit: 16,
        })
        .expect("collect provider document candidates");
    assert_eq!(
        provider_acquisition.candidate_sources,
        vec!["document-element".to_string()]
    );
    assert_eq!(
        provider_acquisition.source_trace[0].source,
        "document-element"
    );
}

fn assert_benchmark_contract(text: &str) {
    for expected in [
        "harness = \"libtest\"",
        "test = \"document_auto_lexical_overlay_warm_path_stays_inside_scenario_gate\"",
        "route_source = \"search-overlay\"",
        "max_provider_process_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(text.contains(expected), "benchmark missing {expected:?}");
    }
}

fn duration_from_manifest(text: &str, field: &str) -> Duration {
    let prefix = format!("{field} = \"");
    let value = text
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| panic!("benchmark missing duration field {field}"));
    if let Some(value) = value.strip_suffix("ms") {
        return Duration::from_millis(value.parse().expect("parse ms duration"));
    }
    if let Some(value) = value.strip_suffix("us") {
        return Duration::from_micros(value.parse().expect("parse us duration"));
    }
    panic!("unsupported benchmark duration {value:?}");
}
