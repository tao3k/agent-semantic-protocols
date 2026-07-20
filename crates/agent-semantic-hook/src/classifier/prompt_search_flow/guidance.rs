pub(super) fn query_projection_flag(language_id: &str) -> &'static str {
    if matches!(language_id, "md" | "org") {
        "--verbatim"
    } else {
        "--code"
    }
}

pub(super) fn invalid_evidence_query_message(language_id: &str, selector: &str) -> String {
    [
        format!("ASP hook denied non-exact evidence query selector `{selector}`."),
        "Evidence projection requires one parser-owned structural item selector.".to_string(),
        String::new(),
        "## Run Next".to_string(),
        format!(
            "Ask `asp-explore` to run `asp {language_id} search owner <owner-path> items --query '<symbol-or-a|b|c>' --workspace . --view seeds` and return selector-only `[asp-search-subagent]` receipts."
        ),
        String::new(),
        "## Rules".to_string(),
        "The parent exact read must use a parser-owned item selector such as `rust://...#item/function/name`."
            .to_string(),
        "Do not use file-level `--code`, line-range selectors, or raw source reads as search evidence."
            .to_string(),
    ]
    .join("\n")
}

pub(super) fn search_flow_feedback_message(
    language_id: &str,
    feedback_kind: &str,
    heading: &str,
    projection_flag: &str,
) -> String {
    match feedback_kind {
        "repeat-search-pipe" => [
            heading.to_string(),
            "The current prompt has already run `search pipe`; pipe is a once-per-prompt frontier."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            "Follow the previous `recommendedNext` / `nextCommand` from the pipe output."
                .to_string(),
            format!(
                "Use the typed `recommendedNext` action, an owner-items query, or `asp {language_id} query --selector '<language>://<owner>#item/<kind>/<name>' --workspace <workspace-root> {projection_flag}`."
            ),
            String::new(),
            "## Rules".to_string(),
            "Do not rerun `search pipe` with a narrower natural term in the same prompt."
                .to_string(),
            format!(
                "Move from frontier to locator/action; keep source reads behind exact `query --selector {projection_flag}`."
            ),
        ]
        .join("\n"),
        "invalid-source-projection-after-pipe" => [
            heading.to_string(),
            "Source projection requires one exact parser-owned structural selector."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            format!(
                "asp {language_id} query --selector '<language>://<owner>#item/<kind>/<name>' --workspace <workspace-root> {projection_flag}"
            ),
            String::new(),
            "## Rules".to_string(),
            "Do not reconstruct path ranges or bypass parser-owned identity.".to_string(),
            format!("Follow locator/frontier evidence and materialize only with exact `query --selector {projection_flag}`."),
        ]
        .join("\n"),
        _ => [
            heading.to_string(),
            "The current prompt has already run `search prime`; prime is only a project map."
                .to_string(),
            String::new(),
            "## Run Next".to_string(),
            "Choose the next ASP route from the current evidence state.".to_string(),
            String::new(),
            "## Rules".to_string(),
            "Follow `recommendedNext` or `nextCommand` when the prime packet supplied one."
                .to_string(),
            format!(
                "Run `asp {language_id} search pipe '<question-or-feature-term>' --workspace . --view seeds` only when the evidence is still ambiguous and needs query refinement."
            ),
            "If an owner, symbol, dependency, test/failure, or exact selector is already known, skip pipe and use the narrower owner/reasoning/query route."
                .to_string(),
            "Do not repeat `search prime`. Do not read source or code before exact parser-owned identity or a route frontier justifies it."
                .to_string(),
        ]
        .join("\n"),
    }
}
