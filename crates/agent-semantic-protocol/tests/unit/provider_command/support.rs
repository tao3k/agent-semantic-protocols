use agent_semantic_client_core::state_core::{ASP_STATE_HOME_ENV, ResolvedState};
use agent_semantic_hook::{builtin_provider_manifests, provider_manifest_digest};
use agent_semantic_runtime::{project_activation_path, project_local_activation_path};
use serde_json::json;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const CACHE_SOURCE_PATH: &str = "src/lib.rs";
pub(super) const CACHE_SOURCE_TEXT: &str = "struct CacheReplay;\n";
pub(super) const CACHE_SOURCE_SHA256: &str =
    "96bc4a7e16de4a4843d4cdf330fabd1448993732fc6a3bec97fed6393a79ecae";

pub(crate) struct ProviderSpec {
    language_id: &'static str,
    command_prefix: Vec<String>,
    owner_items: bool,
    dependency_topology: bool,
}

pub(crate) fn provider(language_id: &'static str, command_prefix: Vec<String>) -> ProviderSpec {
    ProviderSpec {
        language_id,
        command_prefix,
        owner_items: false,
        dependency_topology: false,
    }
}

pub(super) fn provider_with_owner_items(
    language_id: &'static str,
    command_prefix: Vec<String>,
) -> ProviderSpec {
    ProviderSpec {
        language_id,
        command_prefix,
        owner_items: true,
        dependency_topology: false,
    }
}

pub(super) fn provider_with_dependency_topology(
    language_id: &'static str,
    command_prefix: Vec<String>,
) -> ProviderSpec {
    ProviderSpec {
        language_id,
        command_prefix,
        owner_items: false,
        dependency_topology: true,
    }
}

pub(crate) fn write_activation(root: &Path, providers: &[ProviderSpec]) {
    let activation_path =
        project_activation_path(root).unwrap_or_else(|_| project_local_activation_path(root));
    write_activation_to(root, &activation_path, providers);
}

pub(super) fn write_activation_to(root: &Path, activation_path: &Path, providers: &[ProviderSpec]) {
    let activation_dir = activation_path.parent().expect("activation parent");
    std::fs::create_dir_all(activation_dir).expect("create activation dir");
    let providers: Vec<_> = providers
        .iter()
        .map(|spec| {
            let manifest = builtin_provider_manifests()
                .into_iter()
                .find(|manifest| manifest.language_id == spec.language_id)
                .unwrap_or_else(|| panic!("missing manifest for {}", spec.language_id));
            let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
            let mut provider = json!({
                "manifestId": manifest.manifest_id,
                "manifestDigest": manifest_digest,
                "languageId": manifest.language_id,
                "providerId": manifest.provider_id,
                "binary": manifest.binary,
                "execution": manifest.execution,
                "providerCommandPrefix": spec.command_prefix,
                "coverage": {
                    "packageRoots": ["."],
                    "sourceRoots": manifest.source.default_source_roots,
                    "configFiles": manifest.source.default_config_files,
                    "sourceExtensions": manifest.source.default_extensions,
                    "ignoredPathPrefixes": manifest.source.default_ignored_path_prefixes
                }
            });
            if spec.owner_items || spec.dependency_topology {
                provider["searchCapabilities"] = json!({
                    "ownerItems": spec.owner_items,
                    "dependencyTopology": spec.dependency_topology
                });
            }
            if spec.dependency_topology {
                provider["routes"] = json!({
                    "dependencyTopology": {
                        "argv": [manifest.binary]
                    }
                });
            }
            provider
        })
        .collect();
    let activation = json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": root.display().to_string(),
        "generatedBy": { "runtime": "asp", "version": "test" },
        "providers": providers
    });
    std::fs::write(
        activation_path,
        serde_json::to_string_pretty(&activation).expect("serialize activation"),
    )
    .expect("write activation");
}

pub(super) fn assert_compact_search_action_contract(stdout: &str) {
    assert!(
        !stdout.contains("[route-graph]"),
        "default search stdout must not mix graph-frontier output with route-graph debug rows:\n{stdout}"
    );
    assert!(
        !stdout.contains("actionRank="),
        "default search stdout must use compact actionFrontier rows, not actionRank debug rows:\n{stdout}"
    );
    for line in stdout.lines() {
        assert!(
            !is_action_detail_row(line),
            "default search stdout must not expose full action detail rows (`A<n>=kind(...)!suffix`):\n{stdout}"
        );
    }
}

fn is_action_detail_row(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('A') else {
        return false;
    };
    let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digit_count == 0 {
        return false;
    }
    let after_digits = &rest[digit_count..];
    after_digits.starts_with('=') && after_digits.contains('(') && after_digits.contains(")!")
}

pub(super) fn write_rust_owner_frontier_provider(root: &Path) {
    let bin_dir = root.join(".bin");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        "[search-owner] q=src/core.rs pkg=. selector=items alg=item-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,O=owner,I=item}\n\
O=owner:path(src/core.rs)!owner;I=item:symbol(QueryExpr)@src/core.rs:1:1!syntax;I2=item:symbol(parse_query_expr)@src/core.rs:3:3!syntax\n\
syntax I selector=src/core.rs:1:1 pattern='((struct_item name: (_) @type.name) (#eq? @type.name \"QueryExpr\"))'\n\
syntax I2 selector=src/core.rs:3:3 pattern='((function_item name: (_) @function.name) (#eq? @function.name \"parse_query_expr\"))'\n\
G>{O:selects}\n\
O>{I:contains,I2:contains}\n\
rank=I,I2,O frontier=I.syntax,I2.syntax\n\
omit=code,projection-nodes,large-item-text\n\
avoid=inline-code-in-search,raw-read,repeat-owner\n",
        "",
    );
    write_provider_bin_config(root, "rust", &bin_dir.join("rs-harness"));
}

pub(super) fn write_provider_bin_config(root: &Path, language_id: &str, binary: &Path) {
    let config_path = root.join(".agents").join("asp.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create asp config dir");
    std::fs::write(
        config_path,
        format!(
            "[providers.{language_id}]\nbin = \"{}\"\n",
            binary.display().to_string().replace('"', "\\\"")
        ),
    )
    .expect("write asp provider bin config");
}

pub(super) fn write_cache_manifest(root: &Path, manifest: serde_json::Value) -> PathBuf {
    let manifest_path = cache_manifest_path(root);
    std::fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create client cache dir");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("cache manifest JSON"),
    )
    .expect("write cache manifest");
    manifest_path
}

pub(crate) fn cache_root(root: &Path) -> PathBuf {
    resolved_state(root).paths.client_dir
}

pub(super) fn artifacts_root(root: &Path) -> PathBuf {
    resolved_state(root).paths.artifacts_dir
}

pub(super) fn state_home(root: &Path) -> PathBuf {
    root.join("home").join(".agent-semantic-protocols")
}

fn resolved_state(root: &Path) -> ResolvedState {
    ResolvedState::resolve_with_state_home(root, state_home(root)).expect("resolved test state")
}

pub(super) fn cache_manifest_path(root: &Path) -> PathBuf {
    cache_root(root).join("cache-manifest.json")
}

pub(super) fn write_cache_source_fixture(root: &Path) {
    let source_path = root.join(CACHE_SOURCE_PATH);
    std::fs::create_dir_all(source_path.parent().expect("source parent"))
        .expect("create source fixture dir");
    std::fs::write(source_path, CACHE_SOURCE_TEXT).expect("write source fixture");
}

pub(crate) fn asp_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .current_dir(root)
        .env("HOME", root.join("home"))
        .env(ASP_STATE_HOME_ENV, state_home(root))
        .env("ASP_MEMORY_ENGINE_AUTO_SOCKET", "0")
        .env_remove("ASP_MEMORY_ENGINE")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET_DIR")
        .env_remove("PRJ_CACHE_HOME");
    command
}

pub(super) fn home_local_bin(root: &Path) -> PathBuf {
    root.join("home").join(".local/bin")
}

pub(crate) fn prepend_path(path_prefix: &Path) -> OsString {
    let mut paths = vec![path_prefix.to_path_buf()];
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    env::join_paths(paths).expect("join PATH")
}

pub(crate) fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = env::temp_dir().join(format!("agent-semantic-protocol-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    std::fs::create_dir_all(root.join(".git")).expect("create temp git marker");
    root
}

pub(super) fn write_echo_provider(bin_dir: &Path, binary: &str, label: &str) {
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            "#!/bin/sh\nprintf '{label} args='\nfor arg in \"$@\"; do printf '[%s]' \"$arg\"; done\nprintf '\\n'\n"
        ),
    );
}

pub(crate) fn write_marker_provider(bin_dir: &Path, binary: &str, marker: &Path) {
    write_provider_script(
        bin_dir,
        binary,
        &format!("#!/bin/sh\nprintf called > '{}'\n", marker.display()),
    );
}

pub(super) fn write_guide_provider(bin_dir: &Path, binary: &str) {
    write_provider_script(
        bin_dir,
        binary,
        "#!/bin/sh\nprintf '[agent-guide] language=rust provider=rs-harness\\n'\nprintf '|cmd prime=rs-harness search prime .\\n'\nprintf \"|cmd ingest=rg -n '<query>' src tests | rs-harness search ingest .\\n\"\nprintf '|cmd ast-patch=rs-harness ast-patch dry-run --packet <semantic-ast-patch.json|-> .\\n'\nprintf '|cmd evidence=rs-harness evidence graph --review-packet-json <path> --json .\\n'\nprintf '|rule hook setup/runtime is owned by agent-semantic-hook\\n'\n",
    );
}

pub(super) fn write_command_hint_provider(bin_dir: &Path, binary: &str) {
    write_provider_script(
        bin_dir,
        binary,
        "#!/bin/sh\nprintf '{\"provider\":\"rs-harness\",\"next\":\"rs-harness query src/lib.rs .\"}\\n'\n",
    );
}

pub(super) fn write_stdout_stderr_provider(
    bin_dir: &Path,
    binary: &str,
    stdout_text: &str,
    stderr_text: &str,
) {
    write_stdout_stderr_exit_provider(bin_dir, binary, stdout_text, stderr_text, 0);
}

pub(super) fn write_semantic_facts_provider(
    bin_dir: &Path,
    binary: &str,
    stdout_text: &str,
    stderr_text: &str,
) {
    std::fs::create_dir_all(bin_dir).expect("create fake provider bin dir");
    let stdout_path = bin_dir.join(format!("{binary}.semantic-facts.stdout"));
    let stderr_path = bin_dir.join(format!("{binary}.semantic-facts.stderr"));
    std::fs::write(&stdout_path, stdout_text).expect("write semantic facts stdout");
    std::fs::write(&stderr_path, stderr_text).expect("write semantic facts stderr");
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            "#!/bin/sh\ncat >/dev/null\ncat {}\ncat {} >&2\nexit 0\n",
            shell_single_quote(&stdout_path.to_string_lossy()),
            shell_single_quote(&stderr_path.to_string_lossy())
        ),
    );
}

pub(super) fn write_stdout_stderr_exit_provider(
    bin_dir: &Path,
    binary: &str,
    stdout_text: &str,
    stderr_text: &str,
    exit_code: u8,
) {
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            "#!/bin/sh\nprintf '%s' {}\nprintf '%s' {} >&2\nexit {}\n",
            shell_single_quote(stdout_text),
            shell_single_quote(stderr_text),
            exit_code
        ),
    );
}

pub(super) fn write_activation_env_guard_provider(bin_dir: &Path, binary: &str, stdout_text: &str) {
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            "#!/bin/sh\nif [ -n \"${{ASP_PROVIDER_ACTIVATION_PATH:-}}\" ]; then printf 'unexpected client backend activation env\\n' >&2; exit 2; fi\nprintf '%s' {}\n",
            shell_single_quote(stdout_text)
        ),
    );
}

pub(super) fn write_check_failure_provider(bin_dir: &Path, binary: &str, stderr_text: &str) {
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            "#!/bin/sh\ncase \" $* \" in *\" --view \"*) printf 'unexpected --view in provider args\\n' >&2; exit 2;; esac\ncase \" $* \" in *\" --changed \"*) ;; *) printf 'missing --changed in provider args\\n' >&2; exit 2;; esac\nprintf '%s' {} >&2\nexit 1\n",
            shell_single_quote(stderr_text)
        ),
    );
}

pub(super) fn write_pwd_provider(bin_dir: &Path, binary: &str) {
    write_provider_script(bin_dir, binary, "#!/bin/sh\npwd\n");
}

fn shell_single_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn write_provider_script(bin_dir: &Path, binary: &str, text: &str) {
    std::fs::create_dir_all(bin_dir).expect("create fake provider bin dir");
    let path = bin_dir.join(binary);
    std::fs::write(&path, text).expect("write fake provider");
    make_executable(&path);
    if bin_dir.file_name().and_then(|name| name.to_str()) == Some(".bin")
        && let Some(root) = bin_dir.parent()
    {
        let home_bin = home_local_bin(root);
        std::fs::create_dir_all(&home_bin).expect("create fake home-local provider bin dir");
        let home_path = home_bin.join(binary);
        std::fs::write(&home_path, text).expect("write fake home-local provider");
        make_executable(&home_path);
    }
}

pub(super) fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("fake provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod fake provider");
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
