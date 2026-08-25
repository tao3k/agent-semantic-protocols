include!("cli_help_model.rs");
pub(crate) fn install_plugin_command() -> Command {
    Command::new("plugin")
        .bin_name("asp install plugin")
        .about("Install the ASP Codex plugin globally")
        .arg(
            Arg::new("codex")
                .long("codex")
                .required(true)
                .action(ArgAction::SetTrue)
                .help("Target the Codex plugin surface"),
        )
        .arg(
            Arg::new("project-root")
                .value_name("PROJECT_ROOT")
                .help("Explicit ASP source root; defaults to ASP_STATE_HOME [dev].root"),
        )
}

fn paths_command() -> Command {
    Command::new("paths")
        .bin_name("asp paths")
        .about("Resolve ASP state and artifact paths")
        .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
        .arg(Arg::new("get").long("get").value_name("FIELD"))
        .arg(project_root_arg())
}

fn healthcheck_command() -> Command {
    Command::new("healthcheck")
        .bin_name("asp healthcheck")
        .about("Check ASP runtime health")
        .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
}

pub(crate) fn live_corpus_command() -> Command {
    Command::new("live-corpus")
        .bin_name("asp live-corpus")
        .about("Qualify and publish provider live-corpus artifacts")
        .subcommand(
            Command::new("path")
                .about("Resolve the gix-derived canonical State Home checkout path")
                .arg(
                    Arg::new("resource")
                        .long("resource")
                        .value_name("RESOURCE_ID")
                        .required(true),
                )
                .arg(Arg::new("lock").long("lock").value_name("LOCK_JSON"))
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("sync")
                .about("Explicitly acquire and publish a pinned checkout through gix")
                .arg(
                    Arg::new("resource")
                        .long("resource")
                        .value_name("RESOURCE_ID")
                        .required(true),
                )
                .arg(Arg::new("lock").long("lock").value_name("LOCK_JSON"))
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("materialize")
                .about("Qualify a canonical State Home checkout and publish its artifact")
                .arg(
                    Arg::new("resource")
                        .long("resource")
                        .value_name("RESOURCE_ID")
                        .required(true),
                )
                .arg(
                    Arg::new("source")
                        .long("source")
                        .value_name("CHECKOUT")
                        .required(true),
                )
                .arg(Arg::new("lock").long("lock").value_name("LOCK_JSON"))
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("qualify")
                .about(
                    "Measure resident search and exact-query projections for every locked corpus",
                )
                .arg(Arg::new("plan").long("plan").value_name("PLAN_JSON"))
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
}

fn ast_patch_command() -> Command {
    command_with_subcommands(
        "ast-patch",
        "asp ast-patch",
        "Verify or render parser-owned AST patches",
        &[
            ("verify", "Verify an AST patch packet"),
            ("dry-run", "Render an AST patch without applying it"),
            ("template", "Create an AST patch template"),
        ],
    )
    .arg(
        Arg::new("packet")
            .long("packet")
            .value_name("PATH_OR_STDIN"),
    )
    .arg(project_root_arg())
}

fn graph_command() -> Command {
    Command::new("graph")
        .bin_name("asp graph")
        .about("Render ASP evidence graphs")
        .subcommand(
            Command::new("render")
                .about("Render a graph packet")
                .arg(
                    Arg::new("packet")
                        .long("packet")
                        .value_name("PATH_OR_STDIN")
                        .required(true),
                )
                .arg(
                    Arg::new("view")
                        .long("view")
                        .value_name("VIEW")
                        .value_parser(["seeds"]),
                )
                .arg(Arg::new("seeds").long("seeds").value_name("N")),
        )
}

fn facade_command(name: &'static str, bin_name: &'static str) -> Command {
    command_with_subcommands(
        name,
        bin_name,
        "Use a language-owned ASP facade",
        LANGUAGE_COMMANDS,
    )
}

fn facade_leaf_command(name: &'static str, bin_name: &'static str) -> Command {
    let mut command = Command::new(name)
        .bin_name(bin_name)
        .about("Run a language-owned ASP command")
        .arg(
            Arg::new("workspace")
                .long("workspace")
                .value_name("ROOT")
                .help("Select the workspace"),
        );
    if name == "search" {
        command = command
            .arg(
                Arg::new("treesitter-query")
                    .long("treesitter-query")
                    .value_name("QUERY")
                    .help("Run a workspace-wide structural reasoning search"),
            )
            .after_help(
                "Tree-sitter discovery belongs to search. Use query with an exact --selector for deterministic projection.",
            );
    }
    if name == "query" {
        command = command.override_usage(format!(
            "{bin_name} --selector <selector> --projection <source|callable-skeleton> [OPTIONS]"
        ));
    }
    command.arg(
        Arg::new("args")
            .value_name("ARGS")
            .num_args(0..)
            .allow_hyphen_values(true),
    )
}

fn document_facade_command(name: &'static str, bin_name: &'static str) -> Command {
    command_with_subcommands(
        name,
        bin_name,
        "Use a Git-scoped document parser surface",
        DOCUMENT_COMMANDS,
    )
}

fn document_leaf_command(name: &'static str, bin_name: &'static str) -> Command {
    Command::new(name)
        .bin_name(bin_name)
        .about("Run a Git-scoped document parser command")
        .arg(
            Arg::new("workspace")
                .long("workspace")
                .value_name("ROOT")
                .help("Select the Git-scoped workspace"),
        )
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .num_args(0..)
                .allow_hyphen_values(true),
        )
}

fn root_facade_command(language: &str) -> Command {
    match language {
        "gerbil-scheme" => facade_command("gerbil-scheme", "asp gerbil-scheme"),
        "julia" => facade_command("julia", "asp julia"),
        "python" => facade_command("python", "asp python"),
        "rust" => facade_command("rust", "asp rust"),
        "typescript" => facade_command("typescript", "asp typescript"),
        _ => root_command(),
    }
}

fn facade_subcommand(language: &str, subcommand: &'static str) -> Command {
    match language {
        "gerbil-scheme" => facade_leaf_command(subcommand, "asp gerbil-scheme <command>"),
        "julia" => facade_leaf_command(subcommand, "asp julia <command>"),
        "python" => facade_leaf_command(subcommand, "asp python <command>"),
        "rust" => facade_leaf_command(subcommand, "asp rust <command>"),
        "typescript" => facade_leaf_command(subcommand, "asp typescript <command>"),
        _ => root_command(),
    }
}

fn is_language_facade(value: &str) -> bool {
    super::provider_selector::is_language_facade(value)
}

fn is_document_facade(value: &str) -> bool {
    matches!(value, "md" | "org")
}

fn root_document_facade_command(document: &str) -> Command {
    match document {
        "md" => document_facade_command("md", "asp md"),
        "org" => document_facade_command("org", "asp org"),
        _ => root_command(),
    }
}

fn document_subcommand(document: &str, subcommand: &'static str) -> Command {
    match document {
        "md" => document_leaf_command(subcommand, "asp md <command>"),
        "org" => document_leaf_command(subcommand, "asp org <command>"),
        _ => root_command(),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/cli_help.rs"]
mod cli_help_tests;

fn install_language_command() -> Command {
    Command::new("language")
        .bin_name("asp install language")
        .about("Install a language provider globally by default or for one project")
        .arg(Arg::new("language").value_name("LANGUAGE").required(true))
        .arg(
            Arg::new("global")
                .long("global")
                .action(ArgAction::SetTrue)
                .conflicts_with("project")
                .help("Install globally (the default scope)"),
        )
        .arg(Arg::new("target").long("target").value_name("TARGET"))
        .arg(
            Arg::new("record-installed-receipt")
                .long("record-installed-receipt")
                .value_name("BINARY")
                .hide(true),
        )
        .arg(
            Arg::new("project")
                .long("project")
                .value_name("PATH")
                .conflicts_with("global")
                .help("Install only for the project rooted at PATH"),
        )
}

fn graph_render_command() -> Command {
    Command::new("render")
        .bin_name("asp graph render")
        .about("Render a graph packet")
        .arg(
            Arg::new("packet")
                .long("packet")
                .value_name("PATH_OR_STDIN")
                .required(true),
        )
}

fn facade_leaf_name(leaf: &str) -> Option<&'static str> {
    match leaf {
        "guide" => Some("guide"),
        "search" => Some("search"),
        "query" => Some("query"),
        "check" => Some("check"),
        "cache" => Some("cache"),
        "info" => Some("info"),
        "bench" => Some("bench"),
        "projection" => Some("projection"),
        "agent" => Some("agent"),
        "ast-patch" => Some("ast-patch"),
        "evidence" => Some("evidence"),
        _ => None,
    }
}

macro_rules! facade_leaf_bin_for {
    ($language:literal, $leaf:expr) => {
        match $leaf {
            "guide" => Some(concat!("asp ", $language, " guide")),
            "search" => Some(concat!("asp ", $language, " search")),
            "query" => Some(concat!("asp ", $language, " query")),
            "check" => Some(concat!("asp ", $language, " check")),
            "cache" => Some(concat!("asp ", $language, " cache")),
            "info" => Some(concat!("asp ", $language, " info")),
            "bench" => Some(concat!("asp ", $language, " bench")),
            "projection" => Some(concat!("asp ", $language, " projection")),
            "agent" => Some(concat!("asp ", $language, " agent")),
            "ast-patch" => Some(concat!("asp ", $language, " ast-patch")),
            "evidence" => Some(concat!("asp ", $language, " evidence")),
            _ => None,
        }
    };
}

fn facade_leaf_bin(language: &str, leaf: &str) -> Option<&'static str> {
    match language {
        "gerbil-scheme" => facade_leaf_bin_for!("gerbil-scheme", leaf),
        "julia" => facade_leaf_bin_for!("julia", leaf),
        "md" => facade_leaf_bin_for!("md", leaf),
        "org" => facade_leaf_bin_for!("org", leaf),
        "python" => facade_leaf_bin_for!("python", leaf),
        "rust" => facade_leaf_bin_for!("rust", leaf),
        "typescript" => facade_leaf_bin_for!("typescript", leaf),
        _ => None,
    }
}

fn facade_leaf_help(language: &str, leaf: &str) -> Option<Command> {
    Some(
        facade_leaf_command(facade_leaf_name(leaf)?, facade_leaf_bin(language, leaf)?)
            .about("Run a language-provider command"),
    )
}

pub(crate) fn selected_command(args: &[String]) -> Command {
    let path = if args.first().map(String::as_str) == Some("help") {
        &args[1..]
    } else {
        args
    };

    match path {
        [agent, config, sync, ..] if agent == "agent" && config == "config" && sync == "sync" => {
            agent_config_sync_command()
        }
        [install, language, ..] if install == "install" && language == "language" => {
            install_language_command()
        }
        [install, binary, ..] if install == "install" && binary == "binary" => {
            install_binary_command()
        }
        [install, hook, ..] if install == "install" && hook == "hook" => install_hook_command(),
        [hook, break_glass, ..] if hook == "hook" && break_glass == "break-glass" => {
            super::hook_break_glass::break_glass_command()
        }
        [install, plugin, ..] if install == "install" && plugin == "plugin" => {
            install_plugin_command()
        }
        [graph, render, ..] if graph == "graph" && render == "render" => graph_render_command(),
        [document, leaf, ..] if is_document_facade(document) => DOCUMENT_COMMANDS
            .iter()
            .find_map(|(candidate, _)| (*candidate == leaf).then_some(*candidate))
            .map_or_else(
                || root_document_facade_command(document),
                |command| document_subcommand(document, command),
            ),
        [language, leaf, ..] if is_language_facade(language) => {
            facade_leaf_help(language, leaf).unwrap_or_else(|| selected_command_default(path))
        }
        _ => selected_command_default(path),
    }
}

fn selected_command_default(args: &[String]) -> Command {
    let first = args.first().map(String::as_str);
    let second = args.get(1).map(String::as_str);
    match (first, second) {
        (Some("install"), Some("plugin")) => install_plugin_command(),
        (Some("install"), _) => install_command(),
        (Some("hook"), Some("accept-host")) => hook_accept_host_command(),
        (Some("hook"), Some("doctor")) => hook_doctor_command(),
        (Some("hook"), Some("enablement")) => hook_enablement_command(),
        (Some("hook"), Some("break-glass")) => super::hook_break_glass::break_glass_command(),
        (Some("hook"), _) => hook_command(),
        (Some("agent"), Some("config")) => agent_config_command(),
        (Some("agent"), _) => agent_command(),
        (Some("session"), _) => session_control_plane_command(),
        (Some("providers"), _) => providers_command(),
        (Some("tools"), _) => tools_command(),
        (Some("wrap"), _) => wrap_command(),
        (Some("cache"), Some("gc")) => agent_semantic_client::project_registry_gc_clap_command(),
        (Some("cache"), Some("clean")) => {
            agent_semantic_client::project_registry_clean_clap_command()
        }
        (Some("cache"), _) => cache_command(),
        (Some("cloud"), _) => cloud_command(),
        (Some("paths"), _) => paths_command(),
        (Some("healthcheck"), _) => healthcheck_command(),
        (Some("server"), _) => crate::server::runtime_server::runtime_server_command(),
        (Some("schema"), _) => schema_command(),
        (Some("live-corpus"), _) => live_corpus_command(),
        (Some("ast-patch"), _) => ast_patch_command(),
        (Some("graph"), _) => graph_command(),
        (Some("search"), _) => facade_leaf_command("search", "asp search"),
        (Some("query"), _) => facade_leaf_command("query", "asp query"),
        (Some(document), Some(command))
            if is_document_facade(document)
                && DOCUMENT_COMMANDS
                    .iter()
                    .any(|(candidate, _)| *candidate == command) =>
        {
            let command = DOCUMENT_COMMANDS
                .iter()
                .find_map(|(candidate, _)| (*candidate == command).then_some(*candidate))
                .expect("guard requires a registered document command");
            document_subcommand(document, command)
        }
        (Some(document), _) if is_document_facade(document) => {
            root_document_facade_command(document)
        }
        (Some(language), Some(command))
            if is_language_facade(language)
                && LANGUAGE_COMMANDS
                    .iter()
                    .any(|(candidate, _)| *candidate == command) =>
        {
            let command = LANGUAGE_COMMANDS
                .iter()
                .find_map(|(candidate, _)| (*candidate == command).then_some(*candidate))
                .expect("guard requires a registered language command");
            facade_subcommand(language, command)
        }
        (Some(language), _) if is_language_facade(language) => root_facade_command(language),
        (Some("fd"), _) => Command::new("fd")
            .bin_name("asp fd")
            .about("Run the ASP fd compatibility surface"),
        (Some("rg"), _) => Command::new("rg")
            .bin_name("asp rg")
            .about("Run the ASP rg compatibility surface"),
        _ => root_command(),
    }
}

fn print_command_help(mut command: Command) -> Result<(), String> {
    command
        .print_long_help()
        .map_err(|error| format!("failed to print CLI help: {error}"))
}

pub(crate) fn print_help_if_requested(args: &[String]) -> Result<bool, String> {
    let requests_help = args.first().map(String::as_str) == Some("help")
        || args
            .iter()
            .take_while(|arg| arg.as_str() != "--")
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"));
    if !requests_help {
        return Ok(false);
    }

    print_help_if_requested_unchecked(args)
}

fn print_help_if_requested_unchecked(args: &[String]) -> Result<bool, String> {
    let requested = matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h")
    ) || args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"));
    if !requested {
        return Ok(false);
    }
    print_command_help(selected_command(args))?;
    Ok(true)
}

#[allow(dead_code)]
pub(crate) fn print_install_plugin_help() -> Result<(), String> {
    print_command_help(install_plugin_command())
}
#[cfg(test)]
#[path = "../../tests/unit/cli_help_model.rs"]
mod tests;
