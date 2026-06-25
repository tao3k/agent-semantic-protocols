use super::{memory, render, scan};
use std::{
    env,
    path::{Path, PathBuf},
};

pub(crate) fn run_org_recall_command(args: &[String]) -> Result<(), String> {
    let args = OrgRecallArgs::parse(args)?;
    if args.help {
        println!("{}", recall_usage());
        return Ok(());
    }
    match args.command {
        OrgRecallCommand::Plans => recall_plans(args),
    }
}

struct OrgRecallArgs {
    help: bool,
    command: OrgRecallCommand,
    artifacts_root: Option<PathBuf>,
    archive_dir: String,
    state: Option<PathBuf>,
    intent: Option<String>,
    project: Option<String>,
    session: Option<String>,
    branch: Option<String>,
    top_k: Option<String>,
    embedding_dim: Option<String>,
    org_query_bin: Option<String>,
    json: bool,
    include_done: bool,
}

#[derive(Clone, Copy)]
enum OrgRecallCommand {
    Plans,
}

impl OrgRecallArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self {
            help: false,
            command: OrgRecallCommand::Plans,
            artifacts_root: None,
            archive_dir: "archives".to_string(),
            state: None,
            intent: None,
            project: None,
            session: None,
            branch: None,
            top_k: None,
            embedding_dim: None,
            org_query_bin: None,
            json: false,
            include_done: false,
        };
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "-h" | "--help" | "help" => parsed.help = true,
                "plans" if index == 0 => parsed.command = OrgRecallCommand::Plans,
                "--artifacts-root" => {
                    index += 1;
                    parsed.artifacts_root = Some(PathBuf::from(required_flag_value(
                        args,
                        index,
                        "--artifacts-root",
                    )?));
                }
                "--archive-dir" => {
                    index += 1;
                    parsed.archive_dir = required_flag_value(args, index, "--archive-dir")?.into();
                }
                "--state" => {
                    index += 1;
                    parsed.state =
                        Some(PathBuf::from(required_flag_value(args, index, "--state")?));
                }
                "--intent" => {
                    index += 1;
                    parsed.intent = Some(required_flag_value(args, index, "--intent")?.into());
                }
                "--project" => {
                    index += 1;
                    parsed.project = Some(required_flag_value(args, index, "--project")?.into());
                }
                "--session" => {
                    index += 1;
                    parsed.session = Some(required_flag_value(args, index, "--session")?.into());
                }
                "--branch" => {
                    index += 1;
                    parsed.branch = Some(required_flag_value(args, index, "--branch")?.into());
                }
                "--top-k" => {
                    index += 1;
                    parsed.top_k = Some(required_flag_value(args, index, "--top-k")?.into());
                }
                "--embedding-dim" => {
                    index += 1;
                    parsed.embedding_dim =
                        Some(required_flag_value(args, index, "--embedding-dim")?.into());
                }
                "--org-query-bin" => {
                    index += 1;
                    parsed.org_query_bin =
                        Some(required_flag_value(args, index, "--org-query-bin")?.into());
                }
                "--json" => parsed.json = true,
                "--include-done" => parsed.include_done = true,
                _ if arg.starts_with('-') => return Err(format!("unknown recall flag `{arg}`")),
                _ => return Err(format!("unknown recall subcommand `{arg}`")),
            }
            index += 1;
        }
        Ok(parsed)
    }
}

fn recall_plans(args: OrgRecallArgs) -> Result<(), String> {
    let project_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let artifacts_root = match args.artifacts_root {
        Some(path) if path.is_absolute() => path,
        Some(path) => project_root.join(path),
        None => org_artifacts_root_for_project(&project_root),
    };
    let intent = args
        .intent
        .unwrap_or_else(|| "active unfinished ASP Org plan".to_string());
    let project = args
        .project
        .unwrap_or_else(|| "_global_project".to_string());
    let top_k = args
        .top_k
        .as_deref()
        .unwrap_or("5")
        .parse::<usize>()
        .map_err(|error| format!("--top-k must be an integer: {error}"))?;
    let org_query_bin = args.org_query_bin.unwrap_or_else(default_org_query_bin);
    let candidates = scan::scan_org_plan_candidates(
        &artifacts_root,
        &args.archive_dir,
        args.include_done,
        &org_query_bin,
    )?;
    let ranked = memory::rank_plans_with_memory_engine(
        &candidates,
        memory::MemoryRankOptions {
            intent: &intent,
            project: &project,
            session: args.session.as_deref(),
            branch: args.branch.as_deref(),
            state: args.state.as_deref(),
            embedding_dim: args.embedding_dim.as_deref(),
            top_k,
            project_root: &project_root,
        },
    )?;
    if args.json {
        render::print_json_report(&artifacts_root, &args.archive_dir, &ranked)?;
    } else {
        render::print_text_report(&artifacts_root, &args.archive_dir, &ranked);
    }
    Ok(())
}

fn org_artifacts_root_for_project(project_root: &Path) -> PathBuf {
    env::var_os("PRJ_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.join(".cache"))
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org")
}

fn default_org_query_bin() -> String {
    env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "asp".to_string())
}

fn required_flag_value<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn recall_usage() -> &'static str {
    "usage: asp org recall plans [--artifacts-root PATH] [--archive-dir DIR] [--state PATH] [--intent TEXT] [--project ID] [--session ID] [--branch ID] [--top-k N] [--embedding-dim N] [--org-query-bin BIN] [--include-done] [--json]\n\n`recall plans` keeps Org discovery and contract filtering in Rust. Plan candidates come from parser-owned Org query facts. Python asp-memory-engine owns plan ranking, including text, recency, and memory scores. Python does not scan Org files. Repeated agent runs use a resident memory-engine socket by default; set ASP_MEMORY_ENGINE_SOCKET to force a specific worker, ASP_MEMORY_ENGINE_SOCKET_DIR to choose the auto socket directory, or ASP_MEMORY_ENGINE_AUTO_SOCKET=0 to force the direct process fallback. Full cold-start performance runs should set ASP_MEMORY_ENGINE to a packaged binary or provide asp-memory-engine on PATH; build one with `asp-memory-engine build-binary --output .bin/asp-memory-engine`. Local packages/python uv fallback is for development only and is not cold-start performance evidence. The command lists active agent.plan.v1 Org plan ledgers by title, path, recovery command, and score so agents can resume recent unfinished work before archiving DONE records."
}
