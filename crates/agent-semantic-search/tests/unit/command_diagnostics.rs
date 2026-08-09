use agent_semantic_search::command_diagnostics::{
    SearchCommandDiagnostics, take_search_command_diagnostic_options,
};

#[test]
fn search_and_query_share_one_diagnostics_flag_parser() {
    for command in ["search", "query"] {
        let mut args = vec![
            command.to_owned(),
            "lexical".to_owned(),
            "--verbose".to_owned(),
            "--debug".to_owned(),
            "--trace".to_owned(),
            "--workspace".to_owned(),
            ".".to_owned(),
        ];
        let options = take_search_command_diagnostic_options(&mut args)
            .expect("shared diagnostics flags must parse");
        assert!(options.verbose);
        assert!(options.debug);
        assert!(options.trace);
        assert_eq!(
            args,
            [command, "lexical", "--workspace", "."].map(str::to_owned)
        );
    }
}

#[test]
fn diagnostics_flags_are_not_silently_repeated_or_taken_from_provider_data() {
    let mut repeated = vec![
        "search".to_owned(),
        "pipe".to_owned(),
        "--trace".to_owned(),
        "--trace".to_owned(),
    ];
    let error = take_search_command_diagnostic_options(&mut repeated)
        .expect_err("repeated diagnostics flags must fail closed");
    assert!(error.contains("must not be repeated"));

    let mut provider_data = vec!["query".to_owned(), "--".to_owned(), "--trace".to_owned()];
    let options = take_search_command_diagnostic_options(&mut provider_data)
        .expect("provider data delimiter must be preserved");
    assert!(!options.trace);
    assert_eq!(provider_data, ["query", "--", "--trace"].map(str::to_owned));
}

#[test]
fn trace_receipt_is_v1_json_and_marks_budget_excess_as_a_bug() {
    let mut args = vec!["search".to_owned(), "pipe".to_owned(), "--trace".to_owned()];
    let options = take_search_command_diagnostic_options(&mut args).expect("trace flag parses");
    let diagnostics = SearchCommandDiagnostics::start(
        "rust",
        &args,
        options,
        std::time::Instant::now() - std::time::Duration::from_millis(2),
    )
    .expect("search command starts diagnostics");
    let value = serde_json::to_value(diagnostics.finish(1, None))
        .expect("v1 diagnostics receipt is JSON safe");
    assert_eq!(value["schemaVersion"], "1");
    assert_eq!(value["budgetStatus"], "budget-exceeded");
    assert_eq!(value["bug"], true);
    assert!(
        value["stages"]
            .as_array()
            .is_some_and(|stages| !stages.is_empty())
    );
}
