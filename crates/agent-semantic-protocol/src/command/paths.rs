//! Project path introspection for the `asp paths` command.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use agent_semantic_runtime::project_state_paths;
use serde_json::json;

pub(super) fn run_paths_command(args: &[String]) -> Result<(), String> {
    let args = PathsArgs::parse(args)?;
    if args.help {
        println!("{}", usage());
        return Ok(());
    }
    let paths = ProjectPaths::resolve(args.project_root.as_deref())?;
    if let Some(field) = args.get.as_deref() {
        println!("{}", paths.get(field)?);
        return Ok(());
    }
    if args.json {
        println!("{}", paths.to_json());
        return Ok(());
    }
    for (field, value) in paths.fields() {
        println!("{field}={value}");
    }
    Ok(())
}

struct PathsArgs {
    help: bool,
    json: bool,
    get: Option<String>,
    project_root: Option<PathBuf>,
}

impl PathsArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self {
            help: false,
            json: false,
            get: None,
            project_root: None,
        };
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "-h" | "--help" | "help" => parsed.help = true,
                "--json" => parsed.json = true,
                "--get" => {
                    index += 1;
                    parsed.get = Some(required_value(args, index, "--get")?.to_string());
                }
                _ if arg.starts_with("--get=") => {
                    parsed.get = Some(arg.trim_start_matches("--get=").to_string());
                }
                _ if arg.starts_with('-') => return Err(format!("unknown paths flag `{arg}`")),
                _ => {
                    if parsed.project_root.is_some() {
                        return Err("asp paths accepts at most one PROJECT_ROOT".to_string());
                    }
                    parsed.project_root = Some(PathBuf::from(arg));
                }
            }
            index += 1;
        }
        Ok(parsed)
    }
}

struct ProjectPaths {
    fields: BTreeMap<&'static str, String>,
}

impl ProjectPaths {
    fn resolve(project_root: Option<&Path>) -> Result<Self, String> {
        let requested_root =
            project_root
                .map(PathBuf::from)
                .unwrap_or(env::current_dir().map_err(|error| {
                    format!("failed to resolve current directory for asp paths: {error}")
                })?);
        let project_root = requested_root.canonicalize().map_err(|error| {
            format!(
                "failed to resolve project root {}: {error}",
                requested_root.display()
            )
        })?;
        let paths = project_state_paths(&project_root)?;
        let org_state_root = paths.protocol_home.join("org");
        let org_state_skill = org_state_root.join("skills").join("ASP_ORG.org");
        let org_artifacts = paths.artifacts_dir.join("org");
        let org_flow = org_artifacts.join("flow");

        let mut fields = BTreeMap::new();
        fields.insert("projectRoot", path_string(&project_root));
        fields.insert("protocolHome", path_string(&paths.protocol_home));
        fields.insert("hookCacheDir", path_string(&paths.hook_cache_dir));
        fields.insert("hookStateDir", path_string(&paths.hook_state_dir));
        fields.insert("activation", path_string(&paths.activation_path));
        fields.insert("clientCacheDir", path_string(&paths.client_cache_dir));
        fields.insert("artifactsDir", path_string(&paths.artifacts_dir));
        fields.insert("runtimeHome", path_string(&paths.runtime_home));
        fields.insert("runtimeBinDir", path_string(&paths.runtime_bin_dir));
        fields.insert("providerBinDir", path_string(&paths.provider_bin_dir));
        fields.insert("providerLockDir", path_string(&paths.provider_lock_dir));
        fields.insert("orgStateRoot", path_string(&org_state_root));
        fields.insert("orgStateSkill", path_string(&org_state_skill));
        fields.insert("orgArtifacts", path_string(&org_artifacts));
        fields.insert("orgFlow", path_string(&org_flow));
        fields.insert("orgFlowPlans", path_string(&org_flow.join("plans")));
        fields.insert("orgFlowSdd", path_string(&org_flow.join("sdd")));
        fields.insert("orgFlowBdr", path_string(&org_flow.join("BDR")));
        Ok(Self { fields })
    }

    fn get(&self, field: &str) -> Result<&str, String> {
        self.fields
            .get(field)
            .map(String::as_str)
            .ok_or_else(|| format!("unknown asp paths field `{field}`"))
    }

    fn fields(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.fields
            .iter()
            .map(|(field, value)| (*field, value.as_str()))
    }

    fn to_json(&self) -> serde_json::Value {
        json!(self.fields)
    }
}

fn required_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn usage() -> &'static str {
    "usage: asp paths [--json] [--get FIELD] [PROJECT_ROOT]"
}
