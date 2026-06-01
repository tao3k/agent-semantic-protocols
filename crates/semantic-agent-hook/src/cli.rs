//! CLI entrypoint for installing and replaying `semantic-agent-hook` profiles.

use crate::codex_config::{
    ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, codex_hook_block, codex_user_trust_state_status,
    install_codex_user_trust_state, merge_codex_config, validate_codex_config_toml,
};
use crate::{
    ProfileRegistry, classify_hook, merge_profile_registries, parse_payload, parse_profiles,
    render_platform_response,
};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const PYTHON_PROFILE_REGISTRY_JSON: &str = include_str!(
    "../../../languages/python-lang-project-harness/src/python_lang_project_harness/semantic-agent-hook-profile.py-harness.v1.json"
);
const AGENT_SEMANTIC_PROTOCOLS_SKILL_MD: &str = include_str!("../../../SKILL.md");

/// Run the `semantic-agent-hook` CLI using process arguments and standard IO.
pub fn run_cli_from_env() -> Result<(), String> {
    run()
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("hook") => run_hook(&args[1..]),
        Some("doctor") => run_doctor(&args[1..]),
        Some("install") => run_install(&args[1..]),
        Some("profiles") => run_profiles(&args[1..]),
        _ => Err(
            "usage: semantic-agent-hook <install|doctor|hook|profiles> --client codex [PROJECT_ROOT]"
                .to_string(),
        ),
    }
}

fn run_hook(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client")
        .ok_or_else(|| "missing required --client <client>".to_string())?;
    let emit = flag_value(args, "--emit").unwrap_or("platform");
    let event = first_positional(args).ok_or_else(|| "missing hook event".to_string())?;
    let profiles_path = flag_value(args, "--profiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_profile_registry_path(&PathBuf::from(".")));
    let registry = if profiles_path.exists() {
        load_profiles(&profiles_path)?
    } else {
        let value = build_default_profile_registry(&PathBuf::from("."))?;
        parse_profiles(&value.to_string())
            .map_err(|error| format!("invalid generated profile registry: {error:?}"))?
    };
    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .map_err(|error| format!("failed to read hook payload from stdin: {error}"))?;
    let payload =
        parse_payload(&stdin).map_err(|error| format!("invalid hook payload JSON: {error:?}"))?;
    let decision = classify_hook(&registry, client, event, &payload);
    let output_value = match emit {
        "decision" => serde_json::to_value(&decision)
            .map_err(|error| format!("failed to serialize hook decision: {error}"))?,
        "platform" => render_platform_response(&decision)
            .map_err(|error| format!("failed to render hook response: {error:?}"))?,
        other => {
            return Err(format!(
                "unsupported --emit value: {other}; expected platform or decision"
            ));
        }
    };
    let output = serde_json::to_string(&output_value)
        .map_err(|error| format!("failed to serialize hook response: {error}"))?;
    println!("{output}");
    Ok(())
}

fn run_doctor(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_codex_client(client)?;
    let project_root = project_root_arg(args);
    let profiles_path = flag_value(args, "--profiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_profile_registry_path(&project_root));
    let registry = if profiles_path.exists() {
        load_profiles(&profiles_path)?
    } else {
        let value = build_default_profile_registry(&project_root)?;
        parse_profiles(&value.to_string())
            .map_err(|error| format!("invalid generated profile registry: {error:?}"))?
    };
    let config_path = project_root.join(".codex").join("config.toml");
    let config = fs::read_to_string(&config_path).unwrap_or_default();
    let root_hook = config.contains(ROOT_BLOCK_BEGIN) && config.contains(ROOT_BLOCK_END);
    let hook_binary = provider_binary_available("semantic-agent-hook");
    let trust_status = codex_user_trust_state_status(&config_path).ok();
    let trust = trust_status.as_ref().is_some_and(|status| status.trusted);
    let trust_config = trust_status
        .as_ref()
        .map(|status| status.trust_config_path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    println!(
        "[agent-doctor] status=ok client={client} profiles={} profileRegistry={} config={} hook={} trust={} trustConfig={} binary={} protocol={}",
        registry.profiles.len(),
        display_path(&project_root, &profiles_path),
        config_path.is_file(),
        root_hook,
        trust,
        trust_config,
        hook_binary,
        crate::HOOK_PROTOCOL_ID,
    );
    if let Some(status) = trust_status.as_ref() {
        if !status.missing_events.is_empty() {
            println!("|trust missing={}", status.missing_events.join(","));
        }
    }
    for profile in registry.profiles {
        println!(
            "|profile language={} provider={} binary={} roots={} extensions={}",
            profile.language_id,
            profile.provider_id,
            profile.binary,
            profile.source_roots.join(","),
            profile.source_extensions.join(","),
        );
    }
    Ok(())
}

fn run_install(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_codex_client(client)?;
    let project_root = project_root_arg(args);
    let codex_dir = project_root.join(".codex");
    let profiles_path = default_profile_registry_path(&project_root);
    if let Some(parent) = profiles_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let profiles = if let Some(path) = flag_value(args, "--profiles") {
        fs::read_to_string(path)
            .map_err(|error| format!("failed to read profile registry {path}: {error}"))?
    } else {
        serde_json::to_string_pretty(&build_default_profile_registry(&project_root)?)
            .map_err(|error| format!("failed to serialize profile registry: {error}"))?
    };
    parse_profiles(&profiles)
        .map_err(|error| format!("invalid profile registry before install: {error:?}"))?;
    fs::write(&profiles_path, format!("{}\n", profiles.trim_end()))
        .map_err(|error| format!("failed to write {}: {error}", profiles_path.display()))?;
    let skill_path = install_agent_semantic_protocols_skill(&project_root)?;

    fs::create_dir_all(&codex_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_dir.display()))?;
    let config_path = codex_dir.join("config.toml");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    if config_path.is_file() {
        validate_codex_config_toml(&existing)
            .map_err(|error| format!("refusing to write invalid Codex config TOML: {error}"))?;
    }
    let merged = merge_codex_config(&existing, &codex_hook_block());
    validate_codex_config_toml(&merged)
        .map_err(|error| format!("refusing to write invalid Codex config TOML: {error}"))?;
    fs::write(&config_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    let user_config_path = install_codex_user_trust_state(&config_path)?;

    println!(
        "[agent-install] client={client} profiles={} config={} trustConfig={} skill={} binary={} mode=updated",
        display_path(&project_root, &profiles_path),
        display_path(&project_root, &config_path),
        user_config_path.display(),
        display_path(&project_root, &skill_path),
        "semantic-agent-hook",
    );
    Ok(())
}

fn run_profiles(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("merge") => run_profiles_merge(&args[1..]),
        _ => Err(
            "usage: semantic-agent-hook profiles merge --output <path> <profile-registry>..."
                .to_string(),
        ),
    }
}

fn run_profiles_merge(args: &[String]) -> Result<(), String> {
    let output_path = flag_value(args, "--output")
        .ok_or_else(|| "missing required --output <path>".to_string())?;
    let input_paths = positionals(args);
    if input_paths.is_empty() {
        return Err("profiles merge requires at least one profile registry input".to_string());
    }
    let registries = input_paths
        .iter()
        .map(|path| load_profiles(Path::new(path)))
        .collect::<Result<Vec<_>, _>>()?;
    let merged = merge_profile_registries(registries);
    let output = serde_json::to_string_pretty(&merged)
        .map_err(|error| format!("failed to serialize merged profile registry: {error}"))?;
    if let Some(parent) = Path::new(output_path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    fs::write(output_path, format!("{output}\n"))
        .map_err(|error| format!("failed to write {output_path}: {error}"))?;
    println!(
        "[profiles-merge] output={} profiles={}",
        output_path,
        merged.profiles.len()
    );
    Ok(())
}

fn load_profiles(path: &Path) -> Result<ProfileRegistry, String> {
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read profile registry {}: {error}",
            path.display()
        )
    })?;
    parse_profiles(&contents).map_err(|error| format!("invalid profile registry JSON: {error:?}"))
}

fn build_default_profile_registry(_project_root: &Path) -> Result<Value, String> {
    let mut profiles = Vec::new();
    if provider_binary_available("rs-harness") {
        profiles.push(rust_profile());
    }
    if provider_binary_available("ts-harness") {
        profiles.push(typescript_profile());
    }
    if provider_binary_available("py-harness") {
        profiles.push(python_profile());
    }
    if profiles.is_empty() {
        return Err(
            "no semantic hook profiles discovered; expected PATH to contain rs-harness, ts-harness, or py-harness"
                .to_string(),
        );
    }
    Ok(json!({
        "schemaId": crate::PROFILE_REGISTRY_SCHEMA_ID,
        "schemaVersion": crate::PROFILE_REGISTRY_SCHEMA_VERSION,
        "protocolId": crate::HOOK_PROTOCOL_ID,
        "protocolVersion": crate::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "profiles": profiles,
    }))
}

fn provider_binary_available(binary: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|entry| is_executable_file(&entry.join(binary)))
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn rust_profile() -> Value {
    json!({
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "sourceExtensions": [".rs"],
        "configFiles": ["Cargo.toml", "Cargo.lock"],
        "sourceRoots": ["src", "tests", "crates", "examples", "benches", "languages/rust-lang-project-harness/src", "languages/rust-lang-project-harness/tests"],
        "ignoredPathPrefixes": [".cache", ".direnv", ".git", ".idea", ".jj", ".run", ".vscode", "node_modules", "target", ".codex/harness-state", ".codex/rs-harness"],
        "policy": default_policy(),
        "commands": {
            "prime": {"argv": ["rs-harness", "search", "prime", "--view", "seeds", "."]},
            "owner": {"argv": ["rs-harness", "query", "--from-hook", "direct-source-read", "--selector", "{path}", "."]},
            "text": {"argv": ["rs-harness", "search", "text", "{query}", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": ["rs-harness", "search", "ingest", "items", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": ["rs-harness", "check", "--changed", "."]},
            "guide": {"argv": ["rs-harness", "agent", "guide", "."]}
        }
    })
}

fn typescript_profile() -> Value {
    json!({
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "sourceExtensions": [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"],
        "configFiles": ["package.json", "tsconfig.json", "tsconfig.base.json"],
        "sourceRoots": ["src", "test", "tests", "__tests__", "packages", "apps", "lib", "languages/typescript-lang-project-harness/src", "languages/typescript-lang-project-harness/tests"],
        "ignoredPathPrefixes": ["node_modules", "dist", "build", "coverage", ".git"],
        "policy": default_policy(),
        "commands": {
            "prime": {"argv": ["ts-harness", "search", "prime", "."]},
            "owner": {"argv": ["ts-harness", "search", "owner", "{path}", "items", "--query", "{query}", "."]},
            "text": {"argv": ["ts-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": ["ts-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": ["ts-harness", "check", "--changed", "."]},
            "guide": {"argv": ["ts-harness", "agent", "guide", "."]}
        }
    })
}

fn python_profile() -> Value {
    let registry = serde_json::from_str::<Value>(PYTHON_PROFILE_REGISTRY_JSON)
        .expect("Python semantic hook profile registry JSON must be valid");
    registry["profiles"][0].clone()
}

fn default_policy() -> Value {
    json!({
        "directSourceRead": "block",
        "bulkSourceDump": "block",
        "rawSourceSearch": "block",
        "agentSearchJson": "block",
        "blockDirectRead": true,
        "blockBroadRawSearch": true,
        "blockAgentSearchJson": true,
        "requirePrimeBeforeEdit": true,
    })
}

fn default_profile_registry_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".codex")
        .join("semantic-agent-hook")
        .join("profiles.json")
}

fn default_agent_skill_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.md")
}

fn install_agent_semantic_protocols_skill(project_root: &Path) -> Result<PathBuf, String> {
    let skill_path = default_agent_skill_path(project_root);
    if let Some(parent) = skill_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(
        &skill_path,
        format!("{}\n", AGENT_SEMANTIC_PROTOCOLS_SKILL_MD.trim_end()),
    )
    .map_err(|error| format!("failed to write {}: {error}", skill_path.display()))?;
    Ok(skill_path)
}

fn project_root_arg(args: &[String]) -> PathBuf {
    positionals(args)
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn ensure_codex_client(client: &str) -> Result<(), String> {
    if client == "codex" {
        Ok(())
    } else {
        Err(format!("unsupported --client {client}; expected codex"))
    }
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn first_positional(args: &[String]) -> Option<&str> {
    positionals(args).first().copied()
}

fn positionals(args: &[String]) -> Vec<&str> {
    let mut skip_next = false;
    let mut values = Vec::new();
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if matches!(
            arg.as_str(),
            "--client" | "--profiles" | "--emit" | "--output"
        ) {
            skip_next = true;
            continue;
        }
        if !arg.starts_with('-') {
            values.push(arg.as_str());
        }
    }
    values
}
