// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use clap::{Arg, ArgAction, Command};

const ROOT_COMMANDS: &[(&str, &str)] = &[
    ("providers", "Inspect registered language providers"),
    ("tools", "Inspect and run ASP support tools"),
    ("wrap", "Run a command through the ASP client runtime"),
    ("cache", "Inspect and maintain ASP caches"),
    ("cloud", "Inspect optional cloud state"),
    ("hook", "Run and inspect host hook integration"),
    ("config", "Manage ASP-owned global configuration"),
    ("session", "Manage ASP Agent session lifecycle"),
    (
        "install",
        "Install ASP binaries, hooks, plugins, or providers",
    ),
    ("paths", "Resolve ASP state and artifact paths"),
    ("healthcheck", "Check ASP runtime health"),
    ("server", "Manage the Global ASP Runtime Server"),
    (
        "schema",
        "Materialize or verify shared language schema bundles",
    ),
    (
        "live-corpus",
        "Qualify and publish provider live-corpus artifacts",
    ),
    ("ast-patch", "Verify or render parser-owned AST patches"),
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
    ("cache", "Inspect language-owned cache state"),
    ("info", "Show provider information"),
    ("bench", "Run provider benchmarks"),
    ("projection", "Import or inspect language projections"),
    ("agent", "Run provider agent diagnostics"),
    ("ast-patch", "Work with language-owned AST patches"),
];

const DOCUMENT_COMMANDS: &[(&str, &str)] = &[("guide", "Show the document query guide")];

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
}

fn cache_command() -> Command {
    command_with_subcommands(
        "cache",
        "asp cache",
        "Maintain Runtime-owned workspace cache metadata",
        &[(
            "source-index",
            "Use the language-scoped ClientFrame source-index lookup",
        )],
    )
    .arg(
        Arg::new("workspace")
            .long("workspace")
            .value_name("PATH")
            .help("Select the workspace"),
    )
    .subcommand(agent_semantic_client::cache_clean_clap_command())
}

fn cloud_command() -> Command {
    Command::new("cloud")
        .bin_name("asp cloud")
        .about("Inspect optional cloud state")
        .subcommand(Command::new("status").about("Show cloud status"))
}

fn schema_command() -> Command {
    command_with_subcommands(
        "schema",
        "asp schema",
        "Materialize or verify shared language schema bundles",
        &[
            (
                "materialize",
                "Publish Schema Manager-owned language bundles",
            ),
            ("verify", "Verify Schema Manager-owned language bundles"),
        ],
    )
    .arg(
        Arg::new("workspace")
            .long("workspace")
            .value_name("ROOT")
            .help("Select the ASP source workspace"),
    )
    .arg(
        Arg::new("language")
            .long("language")
            .value_name("LANGUAGE_ID")
            .action(ArgAction::Append)
            .help("Select one or more language bundles"),
    )
}

fn hook_command() -> Command {
    command_with_subcommands(
        "hook",
        "asp hook",
        "Run and inspect host hook integration",
        &[
            ("doctor", "Diagnose host hook integration"),
            ("enablement", "Prove that the Codex Hook may be enabled"),
            ("paths", "Resolve hook-owned paths"),
            ("refresh", "Refresh the managed Hook matcher"),
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

fn hook_enablement_command() -> Command {
    Command::new("enablement")
        .bin_name("asp hook enablement")
        .about("Prove that the Codex Hook may be enabled")
        .arg(project_root_arg())
        .arg(
            Arg::new("json")
                .long("json")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("host-rollout")
                .long("host-rollout")
                .value_name("PATH"),
        )
        .arg(
            Arg::new("hook-events")
                .long("hook-events")
                .value_name("PATH"),
        )
        .arg(
            Arg::new("host-probe-path")
                .long("host-probe-path")
                .value_name("PATH"),
        )
        .arg(
            Arg::new("host-sentinel")
                .long("host-sentinel")
                .value_name("TOKEN"),
        )
}

fn config_command() -> Command {
    Command::new("config")
        .bin_name("asp config")
        .about("Manage ASP-owned global configuration")
        .subcommand(agent_config_command())
}

fn session_command() -> Command {
    Command::new("session")
        .bin_name("asp session")
        .about("Manage ASP Agent session lifecycle")
        .subcommand(session_register_child_command())
}

fn session_register_child_command() -> Command {
    Command::new("register-child")
        .bin_name("asp session register-child")
        .about("Register the delivered Codex child under its explicit parent thread")
        .arg(
            Arg::new("parent-thread-id")
                .long("parent-thread-id")
                .value_name("PARENT_THREAD_ID")
                .required(true),
        )
        .arg(
            Arg::new("agent-name")
                .long("agent-name")
                .value_name("CONFIGURED_AGENT_NAME")
                .required(true),
        )
}

fn agent_config_command() -> Command {
    Command::new("agents")
        .bin_name("asp config agents")
        .about("Manage ASP-owned global agent configuration projections")
        .subcommand(agent_config_sync_command())
}

fn agent_config_sync_command() -> Command {
    Command::new("sync")
        .bin_name("asp config agents sync")
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
}
