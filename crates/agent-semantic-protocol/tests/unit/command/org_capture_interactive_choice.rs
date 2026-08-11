use super::AgentInteractiveChoice;
use orgize::Org;

#[test]
fn presentation_choice_requires_all_typed_bindings() {
    let choice = AgentInteractiveChoice::parse(
        "id: session\nmethod: choice\nstage: presentation\ntarget: agent.session.v1\ncreate: deferred\ninfo: choose\ncategories: 1=CALL,?=detail\ndetails:\n|n|id|contract|full|use-if|\n|1|CALL||Call @{{AGENT}} — {{ROLE_DESCRIPTION}}|live binding|",
    )
    .expect("presentation choice");

    let admitted = choice
        .admit_matching(&[
            ("AGENT", "asp_explorer"),
            ("ROLE_DESCRIPTION", "ASP search/query evidence explorer."),
        ])
        .expect("fully bound choice");
    assert_eq!(admitted.len(), 1);
    assert_eq!(
        admitted[0].instruction,
        "Call @asp_explorer — ASP search/query evidence explorer."
    );
    assert!(choice.admit_matching(&[("AGENT", "asp_explorer")]).is_err());
}

#[test]
fn multi_agent_session_contract_owns_every_lifecycle_instruction() {
    let source =
        include_str!("../../../../../org/contracts/agent.multi-agent-session-control-plane.v1.org");
    let org = Org::parse(source);
    let record = org
        .document()
        .source_block_records()
        .into_iter()
        .find(|record| {
            record.language.as_deref() == Some("org-contract")
                && record.header_args.iter().any(|arg| {
                    arg.key == "type" && arg.value.as_deref() == Some("agent-interactive")
                })
        })
        .expect("session agent-interactive contract block");
    let choice = AgentInteractiveChoice::parse(&record.value)
        .expect("session control-plane choice contract");
    let bindings = [
        ("REGISTERED_AGENT_NAME", "asp_testing"),
        ("ROLE_DESCRIPTION", "ASP test/build execution lane."),
        ("SANDBOX_MODE", "read-only"),
    ];

    let registered = choice
        .admit_matching(&[
            ("SESSION_STATE", "registered"),
            ("REGISTERED_AGENT_NAME", "asp_testing"),
            ("ROLE_DESCRIPTION", "ASP test/build execution lane."),
            ("SANDBOX_MODE", "read-only"),
        ])
        .expect("registered contract projection");
    assert_eq!(registered.len(), 1);
    assert_eq!(registered[0].id, "CALL_RESUME_REGISTERED");
    assert_eq!(registered[0].presentation, "action");

    let missing = choice
        .admit_matching(&[
            ("SESSION_STATE", "registration-required"),
            ("REGISTERED_AGENT_NAME", "asp_testing"),
            ("ROLE_DESCRIPTION", "ASP test/build execution lane."),
            ("SANDBOX_MODE", "read-only"),
        ])
        .expect("registration contract projection");
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].id, "CREATE_AND_REGISTER");
    assert!(missing.iter().all(|entry| entry.presentation == "pane"));
    let blocked = choice
        .admit_matching(&[
            ("SESSION_STATE", "blocked"),
            bindings[0],
            bindings[1],
            bindings[2],
        ])
        .expect("blocked contract projection");
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].id, "BLOCKED");
    assert_eq!(blocked[0].presentation, "pane");
    assert!(
        missing
            .iter()
            .all(|entry| entry.instruction.contains("@asp_testing"))
    );
    assert!(
        missing
            .iter()
            .all(|entry| !entry.instruction.contains("asp agent session register"))
    );
    let pane = choice.render_admitted_pane(
        "agent.multi-agent-session-control-plane.v1",
        &[(
            missing[0].id.as_str(),
            missing[0].instruction.as_str(),
            missing[0].use_if.as_str(),
        )],
        "node=registration-required",
    );
    assert!(pane.contains("choice: CREATE_AND_REGISTER"));
    assert!(!pane.contains("\n2."));
    assert!(!pane.contains("choose exactly one"));
    assert!(!pane.contains("do not attach a task payload"));
    assert!(!pane.contains("status=interactive-required"));
    assert!(!pane.contains("entry=not-created"));
    for admitted in registered
        .iter()
        .chain(missing.iter())
        .chain(blocked.iter())
    {
        assert!(admitted.instruction.contains("@asp_testing"));
        assert!(
            admitted
                .instruction
                .contains("ASP test/build execution lane.")
        );
        assert!(admitted.instruction.contains("read-only"));
    }
}

#[test]
fn org_contract_rejects_unknown_presentation_instead_of_rust_inference() {
    let error = match AgentInteractiveChoice::parse(
        "id: session\nmethod: choice\nstage: presentation\ntarget: agent.session.v1\ncreate: deferred\ninfo: choose\ncategories: 1=READY,?=detail\ndetails:\n|n|id|contract|full|when|presentation|use-if|\n|1|READY||Call @{{AGENT}}|STATE=ready|automatic|ready|",
    ) {
        Ok(_) => panic!("unknown presentation directive must fail closed"),
        Err(error) => error,
    };

    assert!(error.contains("presentation must be `action` or `pane`"));
}

#[test]
fn pane_schema_is_generic_and_does_not_reencode_org_choice_plan() {
    let schema = include_str!(
        "../../../../../schemas/multi-agent-session-control-plane-pane.v1.schema.json"
    );

    for org_owned_literal in [
        "CALL_RESUME_REGISTERED",
        "CREATE_AND_REGISTER",
        "BLOCKED",
        "maxItems",
        "prefixItems",
    ] {
        assert!(
            !schema.contains(org_owned_literal),
            "generic pane schema must not own `{org_owned_literal}`"
        );
    }
    assert!(schema.contains("\"presentation\""));
}

#[test]
fn interactive_when_requires_a_typed_binding_instead_of_rust_row_selection() {
    let choice = AgentInteractiveChoice::parse(
        "id: session\nmethod: choice\nstage: presentation\ntarget: agent.session.v1\ncreate: deferred\ninfo: choose\ncategories: 1=READY,2=BLOCKED,?=detail\ndetails:\n|n|id|contract|full|when|use-if|\n|1|READY||Call @{{AGENT}}|STATE=ready|ready|\n|2|BLOCKED||Wait for @{{AGENT}}|STATE=blocked|blocked|",
    )
    .expect("conditional presentation choice");

    let admitted = choice
        .admit_matching(&[("STATE", "ready"), ("AGENT", "asp_explorer")])
        .expect("match typed state");
    assert_eq!(admitted.len(), 1);
    assert_eq!(admitted[0].id, "READY");
    assert!(
        choice
            .admit_matching(&[("AGENT", "asp_explorer")])
            .expect_err("missing typed state must fail")
            .contains("requires missing binding `STATE`")
    );
}
