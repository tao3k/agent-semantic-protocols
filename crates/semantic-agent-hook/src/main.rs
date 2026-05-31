use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

use semantic_agent_hook::{
    classify_hook, merge_profile_registries, parse_payload, parse_profiles,
    render_platform_response,
};
use serde_json::{json, Value};

const ROOT_BLOCK_BEGIN: &str = "# BEGIN semantic-agent-hook agent hooks";
const ROOT_BLOCK_END: &str = "# END semantic-agent-hook agent hooks";
const LEGACY_BLOCKS: [(&str, &str); 3] = [
    (
        "# BEGIN ts-harness agent hooks",
        "# END ts-harness agent hooks",
    ),
    (
        "# BEGIN py-harness agent hooks",
        "# END py-harness agent hooks",
    ),
    (
        "# BEGIN rs-harness agent hooks",
        "# END rs-harness agent hooks",
    ),
];
const CODEX_TOOL_MATCHER: &str = ".*(Read|readFile|readDirectory|read_file|FsReadFile|FsReadDirectory|fs\\.read|fs\\.readDirectory|fs/readFile|fs/readDirectory|fs\\.readbin|writeFile|FsWriteFile|fs\\.write|fs/write|fs\\.writeFile|fs/writeFile|FsRemove|fs\\.remove|fs/remove|FsCopy|fs\\.copy|fs/copy|fs\\.rename|fs/rename|mcp__.*__read.*|Bash|exec_command|command_execution|apply_patch|Edit|Write).*";

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        process::exit(2);
    }
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
    let output = serde_json::to_string_pretty(&output_value)
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
    let local_binary = root_hook_binary_path(&project_root).is_file();
    println!(
        "[agent-doctor] status=ok client={client} profiles={} profileRegistry={} config={} hook={} binary={} protocol={}",
        registry.profiles.len(),
        display_path(&project_root, &profiles_path),
        config_path.is_file(),
        root_hook,
        local_binary,
        semantic_agent_hook::HOOK_PROTOCOL_ID,
    );
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
    let asset_dir = codex_dir.join("semantic-agent-hook");
    let bin_dir = asset_dir.join("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|error| format!("failed to create {}: {error}", bin_dir.display()))?;

    let current_exe =
        env::current_exe().map_err(|error| format!("failed to resolve current exe: {error}"))?;
    let hook_binary = root_hook_binary_path(&project_root);
    fs::copy(&current_exe, &hook_binary).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            current_exe.display(),
            hook_binary.display()
        )
    })?;
    set_executable(&hook_binary)?;

    let profiles_path = default_profile_registry_path(&project_root);
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

    fs::create_dir_all(&codex_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_dir.display()))?;
    let config_path = codex_dir.join("config.toml");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    let merged = merge_codex_config(&existing, &codex_hook_block());
    validate_codex_config_toml(&merged)
        .map_err(|error| format!("refusing to write invalid Codex config TOML: {error}"))?;
    fs::write(&config_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;

    println!(
        "[agent-install] client={client} profiles={} config={} binary={} mode=updated",
        display_path(&project_root, &profiles_path),
        display_path(&project_root, &config_path),
        display_path(&project_root, &hook_binary),
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

fn load_profiles(path: &Path) -> Result<semantic_agent_hook::ProfileRegistry, String> {
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read profile registry {}: {error}",
            path.display()
        )
    })?;
    parse_profiles(&contents).map_err(|error| format!("invalid profile registry JSON: {error:?}"))
}

fn build_default_profile_registry(project_root: &Path) -> Result<Value, String> {
    let mut profiles = Vec::new();
    if project_root.join("Cargo.toml").is_file()
        || project_root
            .join("languages/rust-lang-project-harness/Cargo.toml")
            .is_file()
    {
        profiles.push(rust_profile());
    }
    if project_root.join("package.json").is_file()
        || project_root
            .join("languages/typescript-lang-project-harness/package.json")
            .is_file()
    {
        profiles.push(typescript_profile());
    }
    if project_root.join("pyproject.toml").is_file()
        || project_root
            .join("languages/python-lang-project-harness/pyproject.toml")
            .is_file()
    {
        profiles.push(python_profile());
    }
    if profiles.is_empty() {
        return Err(format!(
            "no semantic hook profiles discovered under {}",
            project_root.display()
        ));
    }
    Ok(json!({
        "schemaId": semantic_agent_hook::PROFILE_REGISTRY_SCHEMA_ID,
        "schemaVersion": semantic_agent_hook::PROFILE_REGISTRY_SCHEMA_VERSION,
        "protocolId": semantic_agent_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": semantic_agent_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "profiles": profiles,
    }))
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
        "ignoredPathPrefixes": ["target", ".git", ".codex/harness-state", ".codex/rs-harness"],
        "policy": default_policy(),
        "commands": {
            "prime": {"argv": ["rs-harness", "search", "prime", "--view", "seeds", "."]},
            "owner": {"argv": ["rs-harness", "search", "owner", "{path}", "items", "--view", "seeds", "."]},
            "text": {"argv": ["rs-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": ["rs-harness", "search", "ingest", "items", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": ["rs-harness", "check", "--changed", "."]}
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
            "owner": {"argv": ["ts-harness", "search", "owner", "{path}", "."]},
            "text": {"argv": ["ts-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": ["ts-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": ["ts-harness", "check", "--changed", "."]}
        }
    })
}

fn python_profile() -> Value {
    json!({
        "languageId": "python",
        "providerId": "py-harness",
        "binary": "py-harness",
        "namespace": "agent.semantic-protocols.languages.python.py-harness",
        "sourceExtensions": [".py", ".pyi"],
        "configFiles": ["pyproject.toml", "setup.cfg", "setup.py"],
        "sourceRoots": ["src", "test", "tests", "packages", "languages/python-lang-project-harness/src", "languages/python-lang-project-harness/tests"],
        "ignoredPathPrefixes": [".venv", "__pycache__", ".git", ".pytest_cache"],
        "policy": default_policy(),
        "commands": {
            "prime": {"argv": ["py-harness", "search", "prime", "."]},
            "owner": {"argv": ["py-harness", "search", "owner", "{path}", "."]},
            "text": {"argv": ["py-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": ["py-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": ["py-harness", "check", "--changed", "."]}
        }
    })
}

fn default_policy() -> Value {
    json!({
        "blockDirectRead": true,
        "blockBroadRawSearch": true,
        "blockAgentSearchJson": true,
        "requirePrimeBeforeEdit": true,
    })
}

fn codex_hook_block() -> String {
    let events = [
        (
            "SessionStart",
            Some("startup|resume|clear|compact"),
            "Loading semantic agent hook profiles",
            "session-start",
        ),
        (
            "UserPromptSubmit",
            None,
            "Planning semantic search flow",
            "user-prompt",
        ),
        (
            "PreToolUse",
            Some(CODEX_TOOL_MATCHER),
            "Checking semantic search flow",
            "pre-tool",
        ),
        (
            "PermissionRequest",
            Some(CODEX_TOOL_MATCHER),
            "Checking semantic approval flow",
            "permission-request",
        ),
        (
            "PostToolUse",
            Some(CODEX_TOOL_MATCHER),
            "Updating semantic search flow state",
            "post-tool",
        ),
        (
            "SubagentStart",
            Some(".*"),
            "Preparing semantic subagent context",
            "subagent-start",
        ),
        (
            "SubagentStop",
            Some(".*"),
            "Checking semantic subagent evidence",
            "subagent-stop",
        ),
        ("Stop", None, "Checking semantic changed files", "stop"),
    ];
    let body = events
        .iter()
        .map(|(event, matcher, status, hook_event)| {
            codex_hook_event_block(event, *matcher, status, hook_event)
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "{ROOT_BLOCK_BEGIN}\n# Generated by `semantic-agent-hook install --client codex`.\n# Root dispatcher for language-owned semantic hook profiles.\n\n{body}\n{ROOT_BLOCK_END}"
    )
}

fn codex_hook_event_block(
    event: &str,
    matcher: Option<&str>,
    status: &str,
    hook_event: &str,
) -> String {
    let matcher_line = matcher
        .map(|value| format!("matcher = {}\n\n", toml_basic_string(value)))
        .unwrap_or_else(|| "\n".to_string());
    format!(
        "[[hooks.{event}]]\n{matcher_line}[[hooks.{event}.hooks]]\ntype = \"command\"\ntimeout = 5\nstatusMessage = \"{status}\"\ncommand = '''\nrepo_root=\"$(git rev-parse --show-toplevel 2>/dev/null || pwd)\"\ncd \"$repo_root\"\nhook_bin=\"$repo_root/.codex/semantic-agent-hook/bin/semantic-agent-hook\"\nprofiles=\"$repo_root/.codex/semantic-agent-hook/profiles.json\"\nif [ -x \"$hook_bin\" ]; then\n  exec \"$hook_bin\" hook --client codex {hook_event} --profiles \"$profiles\"\nfi\nexec semantic-agent-hook hook --client codex {hook_event} --profiles \"$profiles\"\n'''"
    )
}

fn validate_codex_config_toml(content: &str) -> Result<(), String> {
    toml::from_str::<toml::Value>(content)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn toml_basic_string(value: &str) -> String {
    let mut output = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => output.push_str(&format!("\\u{:04X}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn merge_codex_config(existing: &str, block: &str) -> String {
    let mut content = existing.to_string();
    for (begin, end) in LEGACY_BLOCKS {
        content = remove_managed_block(&content, begin, end);
    }
    content = remove_managed_block(&content, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END);
    if !content.contains("unified_exec = true") {
        content = format!("unified_exec = true\n\n{}", content.trim());
    }
    let prefix = content.trim();
    if prefix.is_empty() {
        format!("unified_exec = true\n\n{}\n", block.trim_end())
    } else {
        format!("{}\n\n{}\n", prefix, block.trim_end())
    }
}

fn remove_managed_block(existing: &str, begin: &str, end: &str) -> String {
    let mut content = existing.to_string();
    loop {
        let Some(start) = content.find(begin) else {
            break;
        };
        let Some(relative_end) = content[start..].find(end) else {
            break;
        };
        let end_index = start + relative_end + end.len();
        content.replace_range(start..end_index, "");
    }
    content.trim().to_string()
}

fn default_profile_registry_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".codex")
        .join("semantic-agent-hook")
        .join("profiles.json")
}

fn root_hook_binary_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".codex")
        .join("semantic-agent-hook")
        .join("bin")
        .join("semantic-agent-hook")
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

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to chmod {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
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
