use clap::{Arg, ArgAction, Command};

const ROOT_COMMANDS: &[(&str, &str)] = &[
    ("providers", "Inspect registered language providers"),
    ("tools", "Inspect and run ASP support tools"),
    ("wrap", "Run an ASP-owned tool wrapper"),
    ("cache", "Inspect and maintain ASP caches"),
    ("cloud", "Inspect optional cloud state"),
    ("hook", "Run and inspect host hook integration"),
    ("session", "Open the Multi-Agent Lifecycle control plane"),
    ("agent", "Manage ASP-owned agent configuration projections"),
    (
        "install",
        "Install ASP binaries, hooks, plugins, or providers",
    ),
    ("paths", "Resolve ASP state and artifact paths"),
    ("healthcheck", "Check ASP runtime health"),
    ("server", "Manage the Global ASP Runtime Server"),
    (
        "live-corpus",
        "Qualify and publish provider live-corpus artifacts",
    ),
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
    ("md", "Use the Markdown document surface"),
    ("org", "Use the Org document surface"),
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

const DOCUMENT_COMMANDS: &[(&str, &str)] = &[
    ("guide", "Show the document query guide"),
    ("search", "Discover document-owned structural evidence"),
    ("query", "Query an exact document structural selector"),
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
    .subcommand(agent_semantic_client::project_registry_gc_clap_command())
    .subcommand(agent_semantic_client::project_registry_clean_clap_command())
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
            ("accept-host", "Validate normal-task Host-to-Hook delivery"),
            ("doctor", "Diagnose host hook integration"),
            ("paths", "Resolve hook-owned paths"),
            ("break-glass", "Mint a bound one-shot defect capability"),
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

fn hook_accept_host_command() -> Command {
    Command::new("accept-host")
        .bin_name("asp hook accept-host")
        .about("Validate normal-task Host-to-Hook delivery")
        .arg(
            Arg::new("host-rollout")
                .long("host-rollout")
                .value_name("PATH")
                .required(true),
        )
        .arg(
            Arg::new("host-probe-path")
                .long("host-probe-path")
                .value_name("PATH")
                .required(true),
        )
        .arg(
            Arg::new("host-sentinel")
                .long("host-sentinel")
                .value_name("TOKEN")
                .required(true),
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
        .about("Manage ASP-owned agent configuration projections")
        .subcommand(agent_config_command())
}

fn session_control_plane_command() -> Command {
    Command::new("session")
        .bin_name("asp session")
        .about("Open the current Hook-selected Multi-Agent Lifecycle ChoicePlane")
        .arg(
            Arg::new("agents")
                .long("agents")
                .value_parser(["choice-plane"])
                .required(true),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Render the typed control-plane receipt")
                .action(ArgAction::SetTrue),
        )
}

fn agent_config_command() -> Command {
    Command::new("config")
        .bin_name("asp agent config")
        .about("Manage ASP-owned global agent configuration projections")
        .subcommand(agent_config_sync_command())
}

fn agent_config_sync_command() -> Command {
    Command::new("sync")
        .bin_name("asp agent config sync")
        .about("Reconcile global host agent configuration projections")
}

fn install_command() -> Command {
    Command::new("install")
        .bin_name("asp install")
        .about("Install ASP binaries, hooks, plugins, or providers")
        .subcommand(install_binary_command())
        .subcommand(install_hook_command())
        .subcommand(install_plugin_command())
        .subcommand(install_language_command())
}

fn install_hook_command() -> Command {
    Command::new("hook")
        .bin_name("asp install hook")
        .about("Install host hook integration")
        .arg(
            Arg::new("client")
                .long("client")
                .value_name("CLIENT")
                .required(true)
                .value_parser(["claude"]),
        )
        .arg(project_root_arg())
}

fn install_binary_command() -> Command {
    Command::new("binary")
        .bin_name("asp install binary")
        .about("Install the ASP protocol binary")
        .arg(
            Arg::new("target")
                .long("target")
                .value_name("PATH")
                .required(true),
        )
}
