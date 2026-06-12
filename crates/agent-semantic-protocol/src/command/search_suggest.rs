//! Advisory search command suggestions for ASP-owned search flows.

#[derive(Debug, Eq, PartialEq)]
struct SearchSuggestArgs {
    query: String,
    view: String,
    from_history: bool,
}

pub(super) fn is_search_suggest(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("suggest"))
}

pub(super) fn is_unsupported_search_pipeline_command(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("compose"))
}

pub(super) fn run_search_suggest_command(language_id: &str, args: &[String]) -> Result<(), String> {
    let suggest_args = parse_search_suggest_args(args)?;
    if suggest_args.view != "commands" {
        return Err("search suggest supports --view commands".to_string());
    }
    print!("{}", render_search_suggest(language_id, &suggest_args));
    Ok(())
}

pub(super) fn reject_unsupported_search_pipeline_command() -> Result<(), String> {
    Err(
        "unsupported search pipeline command; use `search pipe` for ASP-owned candidate pipelines"
            .to_string(),
    )
}

fn parse_search_suggest_args(args: &[String]) -> Result<SearchSuggestArgs, String> {
    if !is_search_suggest(args) {
        return Err("expected search suggest command".to_string());
    }
    let query = args
        .get(2)
        .filter(|query| !query.starts_with('-'))
        .ok_or_else(|| "search suggest requires a query".to_string())?
        .clone();
    let mut view = "commands".to_string();
    let mut from_history = false;
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--from-history" => {
                from_history = true;
                index += 1;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search suggest option: {value}"));
            }
            _root_or_scope => {
                // The facade root parser normally removes PROJECT_ROOT before
                // this point. If one remains, keep suggest advisory-only and
                // ignore it rather than treating it as another query term.
                index += 1;
            }
        }
    }
    Ok(SearchSuggestArgs {
        query,
        view,
        from_history,
    })
}

fn render_search_suggest(language_id: &str, args: &SearchSuggestArgs) -> String {
    let query = shell_quote(&args.query);
    let history_mode = if args.from_history {
        "requested"
    } else {
        "optional"
    };
    format!(
        "[search-suggest] lang={language_id} view=commands source=advisory history={history_mode}\n\
|contract executes=false provider=false planner=false output=commands\n\
|history audit=\"asp search history audit .\"\n\
|prefer pipe=\"asp {language_id} search pipe {query} --workspace . --view seeds\"\n\
|prefer fzf=\"asp {language_id} search fzf {query} owner tests --view seeds .\"\n\
|reasoning owner-query=\"asp {language_id} search reasoning owner-query --owner <path> --query {query} --view seeds .\"\n\
|avoid provider-spawn,source-scan,manual-ingest,shell-pipe,natural-language-planning\n"
    )
}

fn shell_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('\'');
    for character in value.chars() {
        if character == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(character);
        }
    }
    quoted.push('\'');
    quoted
}
