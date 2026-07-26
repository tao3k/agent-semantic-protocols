const OPEN_ENVELOPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fixtures/search-interactive-loop/open-envelope.v1.json"
));
const ACTIVE_RUNTIME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fixtures/search-interactive-loop/runtime-active.v1.json"
));
const TERMINAL_RUNTIME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fixtures/search-interactive-loop/runtime-terminal.v1.json"
));

fn unchecked_active() -> crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1 {
    serde_json::from_str(ACTIVE_RUNTIME).expect("active runtime fixture")
}

#[test]
fn validates_open_envelope() {
    let unchecked: crate::search_runtime::UncheckedSearchLoopOpenEnvelopeV1 =
        serde_json::from_str(OPEN_ENVELOPE).expect("open envelope fixture");
    let envelope = crate::search_runtime::SearchLoopOpenEnvelopeV1::validate(unchecked)
        .expect("valid envelope");

    assert_eq!(envelope.language_id(), "rust");
    assert_eq!(envelope.dispatch_ref().as_str(), "dispatch:search-1");
    assert_eq!(
        envelope
            .proposal_set_artifact()
            .artifact_schema_id()
            .as_str(),
        "agent.semantic-protocols.search-route-proposal-set"
    );
}

#[test]
fn validates_active_and_terminal_runtime_bindings() {
    let active = crate::search_runtime::SearchLoopRuntimeBindingV1::validate(unchecked_active())
        .expect("active runtime");
    let terminal = crate::search_runtime::SearchLoopRuntimeBindingV1::validate(
        serde_json::from_str(TERMINAL_RUNTIME).expect("terminal runtime fixture"),
    )
    .expect("terminal runtime");

    assert!(active.active_panel().is_some());
    assert!(active.terminal_binding().is_none());
    assert!(terminal.active_panel().is_none());
    assert!(terminal.terminal_binding().is_some());
}

#[test]
fn rejects_terminal_runtime_with_active_panel() {
    let mut unchecked: crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1 =
        serde_json::from_str(TERMINAL_RUNTIME).expect("terminal runtime fixture");
    unchecked.active_panel = unchecked_active().active_panel;

    assert_eq!(
        crate::search_runtime::SearchLoopRuntimeBindingV1::validate(unchecked).unwrap_err(),
        crate::search_runtime::SearchLoopRuntimeValidationError::TerminalHasActivePanel
    );
}

#[test]
fn rejects_loop_id_aliasing_run_id() {
    let mut unchecked = unchecked_active();
    unchecked.loop_id = unchecked.run_id.clone();

    assert_eq!(
        crate::search_runtime::SearchLoopRuntimeBindingV1::validate(unchecked).unwrap_err(),
        crate::search_runtime::SearchLoopRuntimeValidationError::LoopRunAlias
    );
}

#[test]
fn rejects_references_to_future_revisions() {
    let mut unchecked = unchecked_active();
    unchecked.opened_at_revision = unchecked
        .active_panel
        .as_ref()
        .expect("active panel")
        .based_on_revision
        + 1;

    assert_eq!(
        crate::search_runtime::SearchLoopRuntimeBindingV1::validate(unchecked).unwrap_err(),
        crate::search_runtime::SearchLoopRuntimeValidationError::RevisionBeforeOpen
    );
}

#[test]
fn rejects_duplicate_batch_and_execution_group_bindings() {
    let mut duplicate_batch = unchecked_active();
    duplicate_batch
        .batches
        .push(duplicate_batch.batches[0].clone());
    assert_eq!(
        crate::search_runtime::SearchLoopRuntimeBindingV1::validate(duplicate_batch).unwrap_err(),
        crate::search_runtime::SearchLoopRuntimeValidationError::DuplicateBatchId
    );

    let mut duplicate_group = unchecked_active();
    let mut second = duplicate_group.batches[0].clone();
    second.batch_id = serde_json::from_str("\"batch:parallel-2\"").expect("batch id");
    duplicate_group.batches.push(second);
    assert_eq!(
        crate::search_runtime::SearchLoopRuntimeBindingV1::validate(duplicate_group).unwrap_err(),
        crate::search_runtime::SearchLoopRuntimeValidationError::DuplicateExecutionGroup
    );
}

#[test]
fn serialized_runtime_contains_no_bearer_token_field() {
    let validated = crate::search_runtime::SearchLoopRuntimeBindingV1::validate(unchecked_active())
        .expect("active runtime");
    let serialized =
        serde_json::to_string(&validated.into_unchecked()).expect("serialize runtime binding");

    assert!(!serialized.contains("capability."));
    assert!(!serialized.contains("rawToken"));
    assert!(!serialized.contains("choiceToken"));
    assert!(!serialized.contains("pollToken"));
    assert!(!serialized.contains("receiptToken"));
}
