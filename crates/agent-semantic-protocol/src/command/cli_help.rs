use clap::{Arg, ArgAction, Command};

const ROOT_COMMANDS: &[(&str, &str)] = &[
    ("guide", "Show the ASP command and routing guide"),
    ("providers", "Inspect registered language providers"),
    ("tools", "Inspect and run ASP support tools"),
    ("wrap", "Run an ASP-owned tool wrapper"),
    ("cache", "Inspect and maintain ASP caches"),
    ("cloud", "Inspect optional cloud state"),
    ("hook", "Run and inspect host hook integration"),
    ("agent", "Manage ASP agent sessions"),
    (
        "install",
        "Install ASP binaries, hooks, plugins, or providers",
    ),
    ("sync", "Synchronize project-owned ASP state"),
    ("paths", "Resolve ASP state and artifact paths"),
    ("healthcheck", "Check ASP runtime health"),
    (
        "source-access",
        "Inspect hook-owned source egress decisions",
    ),
    ("ast-patch", "Verify or render parser-owned AST patches"),
    ("graph", "Render ASP evidence graphs"),
    ("fd", "Run the ASP fd compatibility surface"),
    ("rg", "Run the ASP rg compatibility surface"),
    (
        "search",
        "Search with an explicit or inferred language facade",
    ),
    ("query", "Query an exact parser-owned selector"),
    ("gerbil-scheme", "Use the Gerbil Scheme language facade"),
    ("julia", "Use the Julia language facade"),
    ("md", "Use the Markdown language facade"),
    ("org", "Use the Org language facade"),
    ("python", "Use the Python language facade"),
    ("rust", "Use the Rust language facade"),
    ("typescript", "Use the TypeScript language facade"),
];

const LANGUAGE_COMMANDS: &[(&str, &str)] = &[
    ("guide", "Show the language provider guide"),
    ("search", "Search language-owned evidence"),
    ("query", "Query an exact parser-owned selector"),
    ("check", "Run language-owned policy checks"),
    ("cache", "Inspect language-owned cache state"),
    ("info", "Show provider information"),
    ("bench", "Run provider benchmarks"),
    ("projection", "Import or inspect language projections"),
    ("agent", "Run provider agent diagnostics"),
    ("ast-patch", "Work with language-owned AST patches"),
    ("evidence", "Inspect provider evidence"),
];

fn command_with_subcommands(
    name: &'static str,
    bin_name: &'static str,
    about: &'static str,
    subcommands: &'static [(&'static str, &'static str)],
) -> Command {
    subcommands.iter().fold(
        Command::new(name).bin_name(bin_name).about(about),
        |command, (subcommand, description)| {
            command.subcommand(Command::new(*subcommand).about(*description))
        },
    )
}

fn project_root_arg() -> Arg {
    Arg::new("project-root")
        .value_name("PROJECT_ROOT")
        .default_value(".")
        .help("Project root; defaults to the current working directory")
}

fn root_command() -> Command {
    command_with_subcommands(
        "asp",
        "asp",
        "Agent Semantic Protocol command line interface",
        ROOT_COMMANDS,
    )
    .version(env!("CARGO_PKG_VERSION"))
}

fn providers_command() -> Command {
    Command::new("providers")
        .bin_name("asp providers")
        .about("Inspect registered language providers")
        .subcommand(Command::new("list").about("List registered providers"))
        .subcommand(
            Command::new("get")
                .about("Show one registered provider")
                .arg(
                    Arg::new("language-id")
                        .value_name("LANGUAGE_ID")
                        .required(true),
                ),
        )
}

fn tools_command() -> Command {
    Command::new("tools")
        .bin_name("asp tools")
        .about("Inspect and run ASP support tools")
        .subcommand(
            Command::new("doctor")
                .about("Check support-tool availability")
                .arg(project_root_arg()),
        )
        .subcommand(
            Command::new("wrap")
                .about("Run an ASP-owned tool wrapper")
                .arg(
                    Arg::new("tool")
                        .value_name("TOOL")
                        .required(true)
                        .value_parser(["asp-graph-turbo"]),
                )
                .arg(
                    Arg::new("args")
                        .value_name("ARGS")
                        .num_args(0..)
                        .allow_hyphen_values(true),
                ),
        )
}

fn wrap_command() -> Command {
    Command::new("wrap")
        .bin_name("asp wrap")
        .about("Run an ASP-owned tool wrapper")
        .subcommand(
            Command::new("asp-graph-turbo")
                .about("Run the graph-turbo compatibility wrapper")
                .arg(
                    Arg::new("args")
                        .value_name("ARGS")
                        .num_args(0..)
                        .allow_hyphen_values(true),
                ),
        )
}

fn cache_command() -> Command {
    command_with_subcommands(
        "cache",
        "asp cache",
        "Inspect and maintain ASP caches",
        &[
            ("status", "Show cache status"),
            ("import", "Import cache state"),
            ("source-index", "Maintain the source index"),
            ("invalidate", "Invalidate cache state"),
            ("flush", "Flush cache state"),
            ("runtime-source", "Acquire runtime source"),
        ],
    )
    .arg(
        Arg::new("workspace")
            .long("workspace")
            .value_name("PATH")
            .help("Select the workspace"),
    )
}

fn cloud_command() -> Command {
    Command::new("cloud")
        .bin_name("asp cloud")
        .about("Inspect optional cloud state")
        .subcommand(Command::new("status").about("Show cloud status"))
}

fn hook_command() -> Command {
    command_with_subcommands(
        "hook",
        "asp hook",
        "Run and inspect host hook integration",
        &[
            ("doctor", "Diagnose host hook integration"),
            ("paths", "Resolve hook-owned paths"),
            ("pre-tool", "Handle a pre-tool event"),
            ("post-tool", "Handle a post-tool event"),
            ("stop", "Handle a stop event"),
            ("event", "Handle a structured host event"),
        ],
    )
    .arg(
        Arg::new("client")
            .long("client")
            .value_name("CLIENT")
            .value_parser(["codex", "claude"])
            .help("Select the host client"),
    )
}

fn hook_doctor_command() -> Command {
    Command::new("doctor")
        .bin_name("asp hook doctor")
        .about("Diagnose host hook integration")
        .arg(
            Arg::new("client")
                .long("client")
                .value_name("CLIENT")
                .required(true)
                .value_parser(["codex", "claude"]),
        )
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .num_args(0..)
                .allow_hyphen_values(true),
        )
}

fn agent_command() -> Command {
    Command::new("agent")
        .bin_name("asp agent")
        .about("Manage ASP agent sessions")
        .subcommand(agent_session_command())
}

fn agent_session_command() -> Command {
    let mut command = command_with_subcommands(
        "session",
        "asp agent session",
        "Manage the ASP resident-agent session lifecycle",
        &[
            ("bootstrap", "Enter the resident lifecycle loop"),
            ("observe-host-capability", "Record host capability evidence"),
            ("observe-host-tree", "Record host agent-tree evidence"),
            ("observe-host-ack", "Record a live host acknowledgement"),
            ("dispatch-claim", "Claim an exactly-once resident dispatch"),
            ("dispatch-execute", "Execute a claimed resident dispatch"),
            ("dispatch-complete", "Complete a resident dispatch"),
            (
                "dispatch-mark-orphaned",
                "Mark a resident dispatch orphaned",
            ),
            ("register", "Register a resident session"),
            ("list", "List resident sessions"),
            ("show", "Show a resident session"),
            ("status", "Show resident session status"),
            ("smoke", "Run the resident session smoke check"),
            ("resume", "Resume a resident session"),
            ("fork", "Fork a resident session"),
            ("archive", "Archive a resident session"),
            ("close", "Close a resident session"),
            ("gc", "Garbage-collect resident state"),
            ("reconcile", "Reconcile resident state"),
            ("delete", "Delete resident state"),
            ("unarchive", "Unarchive a resident session"),
            ("switch-model", "Switch the resident model"),
        ],
    )
    .subcommand(
        Command::new("lifecycle")
            .about("Inspect resident lifecycle state")
            .subcommand(Command::new("audit").about("Audit resident lifecycle state")),
    );

    for (name, value_name) in [
        ("state-root", "PATH"),
        ("name", "NAME"),
        ("canonical-target", "PATH"),
        ("dispatch-identity", "ID"),
        ("command-digest", "DIGEST"),
        ("command-json", "JSON"),
        ("evidence-ref", "REF"),
        ("schema-digest", "DIGEST"),
        ("child-session-id", "ID"),
        ("message-target-id", "ID"),
        ("root-session-id", "ID"),
        ("parent-session-id", "ID"),
        ("roles", "ROLE[,ROLE...]"),
        ("model", "MODEL"),
        ("status", "STATUS"),
        ("expires-at", "UNIX_TS"),
    ] {
        command = command.arg(
            Arg::new(name)
                .long(name)
                .value_name(value_name)
                .help("Resident session control value"),
        );
    }

    for flag in [
        "guide",
        "resident-bridge",
        "active",
        "replace",
        "force",
        "activity",
        "heartbeat",
        "json",
    ] {
        command = command.arg(
            Arg::new(flag)
                .long(flag)
                .action(ArgAction::SetTrue)
                .help("Resident session control flag"),
        );
    }
    command
}

fn install_command() -> Command {
    Command::new("install")
        .bin_name("asp install")
        .about("Install ASP binaries, hooks, plugins, or providers")
        .subcommand(
            Command::new("binary")
                .about("Install the ASP protocol binary")
                .arg(
                    Arg::new("target")
                        .long("target")
                        .value_name("PATH")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("hook")
                .about("Install host hook integration")
                .arg(
                    Arg::new("client")
                        .long("client")
                        .value_name("CLIENT")
                        .required(true)
                        .value_parser(["claude"]),
                )
                .arg(project_root_arg()),
        )
        .subcommand(install_plugin_command())
        .subcommand(
            Command::new("language")
                .about("Install a language provider")
                .arg(Arg::new("language").value_name("LANGUAGE").required(true))
                .arg(project_root_arg())
                .arg(Arg::new("target").long("target").value_name("TARGET"))
                .arg(Arg::new("project").long("project").value_name("ROOT")),
        )
}

fn install_plugin_command() -> Command {
    Command::new("plugin")
        .bin_name("asp install plugin")
        .about("Install the ASP Codex plugin")
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
                .default_value(".")
                .help("Locate the ASP plugin source from this project root"),
        )
        .arg(
            Arg::new("global")
                .long("global")
                .visible_alias("global-plugin")
                .action(ArgAction::SetTrue)
                .conflicts_with("project")
                .help("Install globally (default when no scope flag is given)"),
        )
        .arg(
            Arg::new("project")
                .long("project")
                .visible_alias("project-plugin")
                .action(ArgAction::SetTrue)
                .conflicts_with("global")
                .help("Also enable and cache the plugin in PROJECT_ROOT"),
        )
        .arg(
            Arg::new("subagent-model")
                .long("subagent-model")
                .value_name("MODEL")
                .help("Override the configured ASP resident subagent model"),
        )
}

fn sync_command() -> Command {
    Command::new("sync")
        .bin_name("asp sync")
        .about("Synchronize project-owned ASP state")
        .arg(project_root_arg())
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
        .arg(project_root_arg())
}

fn source_access_command() -> Command {
    Command::new("source-access")
        .bin_name("asp source-access")
        .about("Inspect hook-owned source egress decisions")
        .subcommand(
            Command::new("shell-egress")
                .about("Report a shell egress decision")
                .arg(
                    Arg::new("activation")
                        .long("activation")
                        .value_name("ACTIVATION_JSON"),
                )
                .arg(
                    Arg::new("command")
                        .long("command")
                        .value_name("COMMAND")
                        .required(true),
                )
                .arg(
                    Arg::new("output-digest")
                        .long("output-digest")
                        .value_name("DIGEST")
                        .required(true),
                )
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
                .arg(Arg::new("path").value_name("PATH").required(true)),
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
    Command::new(name)
        .bin_name(bin_name)
        .about("Run a language-owned ASP command")
        .arg(
            Arg::new("workspace")
                .long("workspace")
                .value_name("ROOT")
                .help("Select the workspace"),
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
        "md" => facade_command("md", "asp md"),
        "org" => facade_command("org", "asp org"),
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
        "md" => facade_leaf_command(subcommand, "asp md <command>"),
        "org" => facade_leaf_command(subcommand, "asp org <command>"),
        "python" => facade_leaf_command(subcommand, "asp python <command>"),
        "rust" => facade_leaf_command(subcommand, "asp rust <command>"),
        "typescript" => facade_leaf_command(subcommand, "asp typescript <command>"),
        _ => root_command(),
    }
}

fn is_language_facade(value: &str) -> bool {
    matches!(
        value,
        "gerbil-scheme" | "julia" | "md" | "org" | "python" | "rust" | "typescript"
    )
}

fn install_language_command() -> Command {
    Command::new("language")
        .bin_name("asp install language")
        .about("Install a language provider")
        .arg(Arg::new("language").value_name("LANGUAGE").required(true))
        .arg(project_root_arg())
        .arg(Arg::new("target").long("target").value_name("TARGET"))
        .arg(Arg::new("project").long("project").value_name("ROOT"))
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
        Command::new(facade_leaf_name(leaf)?)
            .bin_name(facade_leaf_bin(language, leaf)?)
            .about("Run a language-provider command")
            .arg(
                Arg::new("args")
                    .value_name("ARGS")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true),
            ),
    )
}

pub(crate) fn selected_command(args: &[String]) -> Command {
    let path = if args.first().map(String::as_str) == Some("help") {
        &args[1..]
    } else {
        args
    };

    match path {
        [install, language, ..] if install == "install" && language == "language" => {
            install_language_command()
        }
        [graph, render, ..] if graph == "graph" && render == "render" => graph_render_command(),
        [language, leaf, ..] if is_language_facade(language) => {
            facade_leaf_help(language, leaf).unwrap_or_else(|| selected_command_legacy(args))
        }
        _ => selected_command_legacy(args),
    }
}

fn selected_command_legacy(args: &[String]) -> Command {
    let first = args.first().map(String::as_str);
    let second = args.get(1).map(String::as_str);
    match (first, second) {
        (Some("install"), Some("plugin")) => install_plugin_command(),
        (Some("install"), _) => install_command(),
        (Some("hook"), Some("doctor")) => hook_doctor_command(),
        (Some("hook"), _) => hook_command(),
        (Some("agent"), Some("session")) => agent_session_command(),
        (Some("agent"), _) => agent_command(),
        (Some("providers"), _) => providers_command(),
        (Some("tools"), _) => tools_command(),
        (Some("wrap"), _) => wrap_command(),
        (Some("cache"), _) => cache_command(),
        (Some("cloud"), _) => cloud_command(),
        (Some("sync"), _) => sync_command(),
        (Some("paths"), _) => paths_command(),
        (Some("healthcheck"), _) => healthcheck_command(),
        (Some("source-access"), _) => source_access_command(),
        (Some("ast-patch"), _) => ast_patch_command(),
        (Some("graph"), _) => graph_command(),
        (Some("search"), _) => facade_leaf_command("search", "asp search"),
        (Some("query"), _) => facade_leaf_command("query", "asp query"),
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
        (Some("guide"), _) => Command::new("guide")
            .bin_name("asp guide")
            .about("Show the ASP command and routing guide"),
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

pub(crate) fn print_install_plugin_help() -> Result<(), String> {
    print_command_help(install_plugin_command())
}
