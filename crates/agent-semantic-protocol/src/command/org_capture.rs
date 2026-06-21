//! ASP-owned Org capture state materialization.

use agent_semantic_runtime::project_state_paths;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const RESOURCE_DIRS: &[&str] = &["contracts", "templates", "skills"];
const FLOW_DIRS: &[&str] = &["sdd", "BDR", "plans"];

pub(crate) fn run_org_capture_command(args: &[String]) -> Result<(), String> {
    let args = OrgCaptureArgs::parse(args)?;
    if args.help {
        println!("{}", capture_usage());
        return Ok(());
    }

    match args.command {
        OrgCaptureCommand::Init => init_capture_state(args),
    }
}

struct OrgCaptureArgs {
    help: bool,
    command: OrgCaptureCommand,
    source_dir: Option<PathBuf>,
    state_root: Option<PathBuf>,
}

#[derive(Clone, Copy)]
enum OrgCaptureCommand {
    Init,
}

impl OrgCaptureArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self {
            help: false,
            command: OrgCaptureCommand::Init,
            source_dir: None,
            state_root: None,
        };
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "-h" | "--help" | "help" => parsed.help = true,
                "init" if index == 0 => parsed.command = OrgCaptureCommand::Init,
                "--source-dir" => {
                    index += 1;
                    parsed.source_dir = Some(PathBuf::from(required_flag_value(
                        args,
                        index,
                        "--source-dir",
                    )?));
                }
                "--state-root" => {
                    index += 1;
                    parsed.state_root = Some(PathBuf::from(required_flag_value(
                        args,
                        index,
                        "--state-root",
                    )?));
                }
                _ if arg.starts_with('-') => return Err(format!("unknown capture flag `{arg}`")),
                _ => return Err(format!("unknown capture subcommand `{arg}`")),
            }
            index += 1;
        }
        Ok(parsed)
    }
}

fn init_capture_state(args: OrgCaptureArgs) -> Result<(), String> {
    let project_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_root = args.state_root.unwrap_or(
        project_state_paths(&project_root)?
            .protocol_home
            .join("org"),
    );
    let source_root = args.source_dir.unwrap_or_else(default_org_source_root);
    let source_root = source_root.canonicalize().map_err(|error| {
        format!(
            "ASP Org source directory `{}` was not found; set ASP_ORG_SOURCE_DIR or pass --source-dir: {error}",
            source_root.display()
        )
    })?;
    let copied_files = materialize_org_resources(&source_root, &state_root)?;
    let flow_dirs = ensure_flow_dirs(&state_root)?;

    println!("[ASP_ORG_CAPTURE] initialized");
    println!("source-root: {}", display_path(&project_root, &source_root));
    println!("state-root: {}", display_path(&project_root, &state_root));
    println!(
        "skill-entry: {}",
        display_path(
            &project_root,
            &state_root.join("skills").join("ORG_SKILL.org")
        )
    );
    println!(
        "skill-impl: {}",
        display_path(
            &project_root,
            &state_root.join("skills").join("ASP_ORG.org")
        )
    );
    println!(
        "template-plan: {}",
        display_path(
            &project_root,
            &state_root.join("templates").join("agent.plan.v1.org")
        )
    );
    println!(
        "template-execplan: {}",
        display_path(
            &project_root,
            &state_root.join("templates").join("agent.execplan.v1.org")
        )
    );
    println!("flow:");
    for dir in flow_dirs {
        println!("- {}", display_path(&project_root, &dir));
    }
    println!("copied-files: {copied_files}");
    println!("agents-md-include: @.cache/agent-semantic-protocol/org/skills/ORG_SKILL.org");
    println!(
        "next: reference ORG_SKILL.org from AGENTS.md, then write Org state under flow/sdd, flow/BDR, and flow/plans"
    );
    Ok(())
}

fn materialize_org_resources(source_root: &Path, state_root: &Path) -> Result<usize, String> {
    if !source_root.is_dir() {
        return Err(format!(
            "ASP Org source directory `{}` was not found; set ASP_ORG_SOURCE_DIR or pass --source-dir",
            source_root.display()
        ));
    }
    fs::create_dir_all(state_root)
        .map_err(|error| format!("failed to create {}: {error}", state_root.display()))?;

    let mut copied_files = 0;
    for resource in RESOURCE_DIRS {
        let source = source_root.join(resource);
        if !source.is_dir() {
            continue;
        }
        let target = state_root.join(resource);
        if target.exists() {
            fs::remove_dir_all(&target)
                .map_err(|error| format!("failed to refresh {}: {error}", target.display()))?;
        }
        copied_files += copy_tree(&source, &target)?;
    }
    Ok(copied_files)
}

fn ensure_flow_dirs(state_root: &Path) -> Result<Vec<PathBuf>, String> {
    FLOW_DIRS
        .iter()
        .map(|dir| {
            let path = state_root.join("flow").join(dir);
            fs::create_dir_all(&path)
                .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
            Ok(path)
        })
        .collect()
}

fn copy_tree(source: &Path, target: &Path) -> Result<usize, String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
    let mut copied_files = 0;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with(".git") {
            continue;
        }
        let source_path = entry.path();
        let target_path = target.join(name);
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to stat {}: {error}", source_path.display()))?;
        if file_type.is_dir() {
            copied_files += copy_tree(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "failed to copy {} to {}: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
            copied_files += 1;
        }
    }
    Ok(copied_files)
}

fn default_org_source_root() -> PathBuf {
    env::var_os("ASP_ORG_SOURCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../languages/org"))
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

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn capture_usage() -> &'static str {
    "usage: asp org capture init [--source-dir PATH] [--state-root PATH]\n       asp org capture --kind task --title TITLE --target-file ORG_FILE [--outline OUTLINE] [--tag TAG] [--property KEY=VALUE] [--body TEXT]\n\n`capture init` materializes the ASP Org resource tree into .cache/agent-semantic-protocol/org and creates flow/{sdd,BDR,plans}. `capture --kind ...` renders a non-mutating Org entry plan."
}
