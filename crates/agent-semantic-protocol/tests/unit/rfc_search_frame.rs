const SEARCH_FRAME_RFC: &str =
    include_str!("../../../../docs/10-19-rfcs/10.30-search-frame-contract.org");

#[test]
fn search_frame_rfc_defines_frame_boundaries() {
    for expected in [
        "The Search Frame is the first agent-facing frame",
        "WhereFrame answers where to inspect or edit",
        "HowFrame answers how to change safely",
        "must not encode edit groups",
        "validation plans",
    ] {
        assert!(
            SEARCH_FRAME_RFC.contains(expected),
            "Search Frame RFC missing boundary text: {expected}"
        );
    }
}
#[test]
fn search_frame_rfc_defines_evidence_state_router() {
    for expected in [
        "| exact parser selector known |",
        "| owner path known |",
        "| dependency/API known |",
        "| document selector known |",
        "| previous =nextCommand= known |",
        "| hook denies raw source access |",
        "| provider process deadline reached |",
        "=nextCommand=",
        "=nextClasses=",
        "=sourceTrace=",
        "=avoid=",
    ] {
        assert!(
            SEARCH_FRAME_RFC.contains(expected),
            "Search Frame RFC missing router contract: {expected}"
        );
    }
}

#[test]
fn search_frame_rfc_defines_recovery_and_scenario_gates() {
    for expected in [
        "hook_deny_to_asp_explore",
        "parent_wait_deadline_but_child_active",
        "provider_timeout_no_orphan",
        "silent_search_requires_bounded_interrupt_receipt",
        "source_index_stale",
        "dynamic_overlay_dirty_code",
        "document_pipe_lexical_overlay",
        "Hot latency gates follow only after the cold functional gates pass.",
    ] {
        assert!(
            SEARCH_FRAME_RFC.contains(expected),
            "Search Frame RFC missing scenario gate: {expected}"
        );
    }
}
