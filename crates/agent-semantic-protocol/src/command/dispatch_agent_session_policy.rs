pub(crate) fn is_agent_session_direct_inventory_or_fetch_command(args: &[String]) -> bool {
    if !is_agent_session_query_command(args) {
        return false;
    }
    if arg_option_value(args, "--selector").is_none() {
        return false;
    }
    if arg_option_value(args, "--from-hook") == Some("direct-source-read") {
        return false;
    }
    option_is_present(args, "--code")
        || option_is_present(args, "--content")
        || arg_option_value(args, "--view") == Some("metadata")
}

fn is_agent_session_query_command(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("query"))
        || args
            .get(1)
            .is_some_and(|stage| matches!(stage.as_str(), "query"))
}

fn arg_option_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix))
        .or_else(|| {
            args.windows(2)
                .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
        })
}

fn option_is_present(args: &[String], option: &str) -> bool {
    args.iter().any(|arg| arg == option)
}
