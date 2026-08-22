use agent_semantic_client_core::state_core::{ASP_STATE_HOME_ENV, ResolvedState};
use agent_semantic_hook::{
    builtin_provider_manifests, provider_execution_command_digest, provider_manifest_digest,
};
use serde_json::json;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod paths;
#[cfg(test)]
mod tests;

pub(super) const CACHE_SOURCE_PATH: &str = "src/lib.rs";
pub(super) const CACHE_SOURCE_TEXT: &str = "struct CacheReplay;\n";
pub(super) const CACHE_SOURCE_SHA256: &str =
    "96bc4a7e16de4a4843d4cdf330fabd1448993732fc6a3bec97fed6393a79ecae";

pub(crate) struct ProviderSpec {
    language_id: String,
    dependency_topology_fixture: bool,
}

pub(crate) fn provider(language_id: impl AsRef<str>, command_prefix: Vec<String>) -> ProviderSpec {
    assert!(
        command_prefix.is_empty(),
        "State Home v1 activation fixtures cannot embed provider command prefixes: commandPrefix={command_prefix:?}"
    );
    ProviderSpec {
        language_id: language_id.as_ref().to_string(),
        dependency_topology_fixture: false,
    }
}

pub(crate) fn provider_with_owner_items(
    language_id: &'static str,
    command_prefix: Vec<String>,
) -> ProviderSpec {
    provider(language_id, command_prefix)
}

pub(super) fn provider_with_dependency_topology(
    language_id: &'static str,
    command_prefix: Vec<String>,
) -> ProviderSpec {
    let mut spec = provider(language_id, command_prefix);
    spec.dependency_topology_fixture = true;
    spec
}

pub(crate) fn write_activation(root: &Path, providers: &[ProviderSpec]) {
    let activation_path = state_activation_path(root);
    write_activation_to(root, &activation_path, providers);
}

pub(super) fn write_activation_to(root: &Path, activation_path: &Path, providers: &[ProviderSpec]) {
    let activation_dir = activation_path.parent().expect("activation parent");
    std::fs::create_dir_all(activation_dir).expect("create activation dir");
    let runtime_bin = state_runtime_bin(root);
    std::fs::create_dir_all(&runtime_bin).expect("create fixture runtime bin");
    let runtime_asp = runtime_bin.join("asp");
    if !runtime_asp.exists() {
        std::fs::copy(env!("CARGO_BIN_EXE_asp"), &runtime_asp)
            .expect("install fixture canonical ASP runtime");
    }
    let providers: Vec<_> = providers
        .iter()
        .map(|spec| {
            let manifest = builtin_provider_manifests()
                .into_iter()
                .find(|manifest| manifest.language_id().as_str() == spec.language_id)
                .unwrap_or_else(|| panic!("missing manifest for {}", spec.language_id));
            let runtime_bin = state_runtime_bin(root);
            let installed_provider = runtime_bin.join(manifest.binary());
            if !installed_provider.exists() {
                write_marker_provider(
                    &runtime_bin,
                    manifest.binary(),
                    &runtime_bin.join(format!(".{}-marker", manifest.binary())),
                );
            }
            let artifact_digest =
                agent_semantic_content_identity::file_content_digest_v1(&installed_provider)
                    .expect("installed provider artifact digest");
            let resolved_execution_prefix = vec![installed_provider.display().to_string()];
            let execution_command_digest =
                provider_execution_command_digest(&resolved_execution_prefix, &artifact_digest)
                    .expect("provider execution command digest");
            let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
            let mut routes = serde_json::to_value(
                agent_semantic_hook::materialize_provider_routes(&manifest)
                    .expect("materialize provider routes"),
            )
            .expect("serialize provider routes");
            let mut search_capabilities = serde_json::to_value(manifest.search_capabilities())
                .expect("serialize search capabilities");
            if spec.dependency_topology_fixture {
                search_capabilities["dependencyTopology"] = json!(true);
                routes["dependencyTopology"] = json!({
                    "argv": [
                        manifest.binary(),
                        "search",
                        "dependency-topology",
                        "--json",
                        "--workspace",
                        "{workspace}"
                    ]
                });
            }
            let provider = json!({
                "manifestId": manifest.manifest_id(),
                "manifestDigest": manifest_digest,
                "languageId": manifest.language_id(),
                "providerId": manifest.provider_id(),
                "binary": manifest.binary(),
                "execution": manifest.execution(),
                "providerCommandPrefix": [],
                "executionCommandDigest": execution_command_digest,
                "searchCapabilities": search_capabilities,
                "semanticFactsDescriptor": manifest.semantic_facts_descriptor(),
                "queryPackDescriptor": manifest.query_pack_descriptor(),
                "semanticRegistryDigest": agent_semantic_hook::semantic_registry_digest(),
                "routes": routes,
                "coverage": {
                    "packageRoots": [],
                    "configFiles": crate::provider_manifest_scope::project_entries(&manifest),
                    "sourceExtensions": crate::provider_manifest_scope::document_extensions(&manifest)
                }
            });
            provider
        })
        .collect();
    let activation = json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "schemaAuthority": "https://tao3k.github.io/agent-semantic-protocols/schemas/",
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": root.display().to_string(),
        "generatedBy": { "runtime": "asp", "version": "test" },
        "rankers": [],
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
    let legacy_stdout = "[search-owner] q=src/core.rs pkg=. selector=items alg=item-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,O=owner,I=item}\n\
O=owner:path(src/core.rs)!owner;I=item:symbol(QueryExpr)@src/core.rs:1:1!syntax;I2=item:symbol(parse_query_expr)@src/core.rs:3:3!syntax\n\
syntax I selector=src/core.rs:1:1 pattern='((struct_item name: (_) @type.name) (#eq? @type.name \"QueryExpr\"))'\n\
syntax I2 selector=src/core.rs:3:3 pattern='((function_item name: (_) @function.name) (#eq? @function.name \"parse_query_expr\"))'\n\
G>{O:selects}\n\
O>{I:contains,I2:contains}\n\
rank=I,I2,O frontier=I.syntax,I2.syntax\n\
omit=code,projection-nodes,large-item-text\n\
avoid=inline-code-in-search,raw-read,repeat-owner\n";
    write_provider_script(
        &bin_dir,
        "rs-harness",
        &format!(
            "#!/bin/sh\n\
native_owner=0\n\
for arg in \"$@\"; do\n\
  if [ \"$arg\" = 'owner-search-stdin' ]; then native_owner=1; fi\n\
done\n\
if [ \"$native_owner\" = '1' ]; then\n\
  request=$(cat)\n\
  digest=$(printf '%s' \"$request\" | sed -n 's/.*\"contentDigest\":\"\\([0-9a-f]*\\)\".*/\\1/p')\n\
  printf '%s\\n' \"{{\\\"schemaId\\\":\\\"agent.semantic-protocols.provider-native-owner-search-response\\\",\\\"schemaVersion\\\":\\\"1\\\",\\\"languageId\\\":\\\"rust\\\",\\\"providerId\\\":\\\"rs-harness\\\",\\\"requestedOwnerPath\\\":\\\"src/core.rs\\\",\\\"requestedQuery\\\":\\\"QueryExpr\\\",\\\"sourceContentDigest\\\":\\\"$digest\\\",\\\"parsedOwnerCount\\\":1,\\\"projectionCompleteness\\\":\\\"complete-owner\\\",\\\"projections\\\":[{{\\\"structuralSelector\\\":\\\"rust://src/core.rs#item/struct/QueryExpr\\\",\\\"itemKind\\\":\\\"struct\\\",\\\"itemName\\\":\\\"QueryExpr\\\",\\\"captureName\\\":\\\"type.name\\\",\\\"signature\\\":\\\"struct QueryExpr\\\",\\\"sourceByteStart\\\":0,\\\"sourceByteEnd\\\":16}},{{\\\"structuralSelector\\\":\\\"rust://src/core.rs#item/function/parse_query_expr\\\",\\\"itemKind\\\":\\\"function\\\",\\\"itemName\\\":\\\"parse_query_expr\\\",\\\"captureName\\\":\\\"function.name\\\",\\\"signature\\\":\\\"fn parse_query_expr\\\",\\\"sourceByteStart\\\":17,\\\"sourceByteEnd\\\":36}}]}}\"\n\
  exit 0\n\
fi\n\
printf '%s' {}\n",
            shell_single_quote(legacy_stdout)
        ),
    );
    install_state_home_provider(root, "rust", &bin_dir.join("rs-harness"));
}

pub(crate) fn install_state_home_provider(root: &Path, language_id: &str, binary: &Path) {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .unwrap_or_else(|| panic!("missing manifest for {language_id}"));
    let installed = state_runtime_bin(root).join(manifest.binary());
    std::fs::create_dir_all(installed.parent().expect("provider install parent"))
        .expect("create state-home runtime bin");
    std::fs::copy(binary, &installed).expect("install test provider in state home");
    make_executable(&installed);
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

fn state_activation_path(root: &Path) -> PathBuf {
    resolved_state(root)
        .paths
        .hooks_dir
        .join("state")
        .join("activation.json")
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
    let runtime_bin = state_runtime_bin(root);
    write_default_project_resolution_provider_shims(&runtime_bin);
    write_provider_install_receipts(root);
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .current_dir(root)
        .env("HOME", root.join("home"))
        .env(ASP_STATE_HOME_ENV, state_home(root))
        .env("ASP_MEMORY_ENGINE_AUTO_SOCKET", "0")
        .env_remove("ASP_MEMORY_ENGINE")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET_DIR")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CODEX_PARENT_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .env_remove("PRJ_CACHE_HOME");
    command
}

pub(crate) struct RuntimeServerGuard {
    root: PathBuf,
}

impl Drop for RuntimeServerGuard {
    fn drop(&mut self) {
        let _ = asp_command(&self.root).args(["server", "stop"]).status();
    }
}

pub(crate) async fn start_runtime_server(root: &Path) -> RuntimeServerGuard {
    let output = super::runtime_support::run_runtime_server_start(root)
        .await
        .expect("start isolated Runtime Server");
    assert!(
        output.status.success(),
        "start isolated Runtime Server: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    RuntimeServerGuard {
        root: root.to_path_buf(),
    }
}

pub(crate) fn admit_runtime_resident_generation(root: &Path) {
    super::runtime_support::admit_runtime_resident_generation(root);
}

pub(super) fn state_runtime_bin(root: &Path) -> PathBuf {
    state_home(root).join("runtime/bin")
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
    let git_status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("initialize temp git project");
    assert!(git_status.success(), "initialize temp git project");
    std::fs::create_dir_all(root.join("src")).expect("create temp source root");
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
            name.replace('_', "-")
        ),
    )
    .expect("write temp Cargo manifest");
    std::fs::write(root.join("src/lib.rs"), "").expect("write temp Rust source");
    root
}

pub(crate) fn write_echo_provider(bin_dir: &Path, binary: &str, label: &str) {
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            "#!/bin/sh\nprintf '{label} args='\nfor arg in \"$@\"; do printf '[%s]' \"$arg\"; done\nprintf '\\n'\n"
        ),
    );
}

pub(crate) fn write_recording_provider(
    root: &Path,
    bin_dir: &Path,
    language_id: &str,
    binary: &str,
    label: &str,
    marker: &Path,
) {
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            "#!/bin/sh\nprintf '{label} args=' > '{}'\nfor arg in \"$@\"; do printf '[%s]' \"$arg\" >> '{}'; done\nprintf '\\n' >> '{}'\nprintf '{label} args='\nfor arg in \"$@\"; do printf '[%s]' \"$arg\"; done\nprintf '\\n'\n",
            marker.display(),
            marker.display(),
            marker.display(),
        ),
    );
    install_state_home_provider(root, language_id, &bin_dir.join(binary));
}

pub(crate) fn write_marker_provider(bin_dir: &Path, binary: &str, marker: &Path) {
    let delegate = bin_dir.join(format!(".{binary}-delegate"));
    let (language_id, provider_id, source_extensions) = match binary {
        "rs-harness" => ("rust", binary, r#"[".rs"]"#),
        "asp-typescript" => (
            "typescript",
            binary,
            r#"[".ts",".tsx",".js",".jsx",".mts",".cts",".mjs",".cjs"]"#,
        ),
        "python-harness" | "py-harness" => ("python", binary, r#"[".py"]"#),
        "julia-harness" | "asp-julia-harness" => ("julia", binary, r#"[".jl"]"#),
        "gslph" => (
            "gerbil-scheme",
            "gerbil-scheme-harness",
            r#"[".ss",".ssi",".scm",".sld"]"#,
        ),
        _ => ("test", binary, r#"[".txt"]"#),
    };
    let candidate_generation_digest = format!(
        "blake3-256:{}",
        blake3::hash(
            format!(
                "provider-project-resolution-v1\0{language_id}\0{provider_id}\0{source_extensions}\0fixture-package\0fixture-lib\0fixture-root"
            )
            .as_bytes()
        )
        .to_hex()
    );
    let package_graph = serde_json::to_string(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.language-package-graph",
        "schemaVersion": "1",
        "languageId": language_id,
        "providerId": provider_id,
        "projectEntry": "Cargo.toml",
        "parserId": provider_id,
        "manifests": [{
            "path": "Cargo.toml",
            "kind": "manifest",
            "digest": candidate_generation_digest,
        }],
        "lockfiles": [],
        "packages": [{
            "packageId": "fixture-package",
            "name": "fixture-package",
            "manifestPath": "Cargo.toml",
            "root": ".",
            "workspaceMember": true,
            "targets": [{
                "targetId": "fixture-lib",
                "kind": "lib",
                "name": "fixture-lib",
                "explicit": true,
                "sourceRoots": ["."],
                "entrypoints": ["lib.rs"],
                "generatedRoots": [],
            }],
        }],
        "internalDependencyEdges": [],
        "externalDependencies": [],
        "unresolved": [],
    }))
    .expect("serialize fixture language package graph");
    write_provider_script(
        bin_dir,
        binary,
        &format!(
            r#"#!/bin/sh
# agent-semantic-protocol-test-project-resolution-shim-v1
if [ "$1" = "project-resolution-stdin" ]; then
  printf '{{"schemaId":"agent.semantic-protocols.provider-project-resolution-response","schemaVersion":"1","languageId":"{language_id}","providerId":"{provider_id}","state":"resolved","scope":{{"schemaId":"agent.semantic-protocols.project-resolution","schemaVersion":"1","state":"resolved","completeness":"exact","languageId":"{language_id}","providerId":"{provider_id}","parserId":"{provider_id}","candidateGenerationDigest":"{candidate_generation_digest}","projectEntry":"Cargo.toml","packageGraph":{package_graph},"conflicts":[],"metrics":{{"parsedManifestCount":1,"parsedLockfileCount":0,"affectedPackageCount":1,"fullWorkspaceReads":0,"fullManifestReparses":0,"dbOpens":0,"elapsedMicros":0}},"sourceScopes":[{{"scopeId":"fixture-root","packageId":"fixture-package","targetId":"fixture-lib","roots":["."],"explicitPaths":[],"extensions":{source_extensions},"includeAuthority":"package-manager","exclusions":[]}}]}}}}\n'
  exit 0
fi
if [ -x '{}' ]; then
  exec '{}' "$@"
fi
printf called > '{}'
"#,
            delegate.display(),
            delegate.display(),
            marker.display()
        ),
    );
}

fn write_default_project_resolution_provider_shims(bin_dir: &Path) {
    std::fs::create_dir_all(bin_dir).expect("create default provider shim directory");
    for binary in [
        "rs-harness",
        "asp-typescript",
        "py-harness",
        "asp-julia-harness",
        "gslph",
    ] {
        let provider = bin_dir.join(binary);
        if !provider.exists() {
            continue;
        }
        if is_default_project_resolution_provider_shim(&provider) {
            continue;
        }
        let delegate = bin_dir.join(format!(".{binary}-delegate"));
        if delegate.exists() {
            std::fs::remove_file(&delegate).expect("replace test provider delegate");
        }
        std::fs::rename(&provider, &delegate).expect("preserve test provider delegate");
        let marker = bin_dir.join(format!(".{binary}-marker"));
        write_marker_provider(bin_dir, binary, &marker);
    }
}

fn is_default_project_resolution_provider_shim(provider: &Path) -> bool {
    std::fs::read_to_string(provider)
        .ok()
        .is_some_and(|source| source.starts_with(DEFAULT_PROJECT_RESOLUTION_PROVIDER_SHIM_PREFIX))
}

const DEFAULT_PROJECT_RESOLUTION_PROVIDER_SHIM_PREFIX: &str =
    "#!/bin/sh\n# agent-semantic-protocol-test-project-resolution-shim-v1\n";

pub(super) fn write_guide_provider(bin_dir: &Path, binary: &str) {
    write_provider_script(
        bin_dir,
        binary,
        "#!/bin/sh\nprintf '[agent-guide] language=rust provider=rs-harness\\n'\nprintf '|cmd lexical=rs-harness search lexical --query <seed> --query <seed> owner tests --workspace . --view seeds\\n'\nprintf \"|cmd ingest=rg -n '<query>' src tests | rs-harness search ingest --workspace .\\n\"\nprintf '|cmd ast-patch=rs-harness ast-patch dry-run --packet <semantic-ast-patch.json|-> --workspace .\\n'\nprintf '|cmd evidence=rs-harness evidence graph --review-packet-json <path> --json --workspace .\\n'\nprintf '|rule hook setup/runtime is owned by agent-semantic-hook\\n'\n",
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
    let semantic_facts_script = format!(
        "#!/bin/sh\ncat >/dev/null\ncat {}\ncat {} >&2\nexit 0\n",
        shell_single_quote(&stdout_path.to_string_lossy()),
        shell_single_quote(&stderr_path.to_string_lossy())
    );
    write_provider_script(
        bin_dir,
        &format!(".{binary}-delegate"),
        &semantic_facts_script,
    );
    write_marker_provider(bin_dir, binary, &bin_dir.join(format!(".{binary}-marker")));
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
    if !bin_dir.ends_with("runtime/bin")
        && builtin_provider_manifests()
            .into_iter()
            .any(|manifest| manifest.binary() == binary)
        && let Some(root) = bin_dir.parent()
    {
        let installed = state_runtime_bin(root).join(binary);
        std::fs::create_dir_all(installed.parent().expect("provider install parent"))
            .expect("create state-home runtime bin");
        std::fs::copy(&path, &installed).expect("install provider fixture in state home");
        let permissions = std::fs::metadata(&path)
            .expect("provider fixture metadata")
            .permissions();
        std::fs::set_permissions(&installed, permissions)
            .expect("preserve installed provider fixture permissions");
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
    if path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some(".bin")
        && let Some(root) = path.parent().and_then(Path::parent)
        && let Some(file_name) = path.file_name()
    {
        let installed = state_runtime_bin(root).join(file_name);
        std::fs::create_dir_all(installed.parent().expect("provider install parent"))
            .expect("create state-home runtime bin");
        std::fs::copy(path, &installed).expect("install executable fixture in state home");
        let permissions = std::fs::metadata(path)
            .expect("fixture metadata")
            .permissions();
        std::fs::set_permissions(&installed, permissions)
            .expect("preserve installed fixture permissions");
    }
}

fn write_provider_install_receipts(root: &Path) {
    let state_home = state_home(root);
    let runtime_bin = state_home.join("runtime/bin");
    let provider_lock_dir = agent_semantic_runtime::provider_receipt_dir(&state_home);
    std::fs::create_dir_all(&provider_lock_dir).expect("create provider lock registry");
    for manifest in builtin_provider_manifests() {
        let installed = runtime_bin.join(manifest.binary());
        if !installed.is_file() {
            continue;
        }
        let content_digest = agent_semantic_content_identity::file_content_digest_v1(&installed)
            .expect("installed provider content digest");
        let metadata_digest =
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed)
                .expect("installed provider metadata digest");
        let lock_path = provider_lock_dir.join(format!("{}.lock.toml", manifest.language_id()));
        std::fs::write(
            lock_path,
            format!(
                "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{}\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\n",
                manifest.language_id(),
                manifest.provider_id(),
                installed.display(),
                content_digest,
                metadata_digest,
            ),
        )
        .expect("write provider install receipt");
    }
}
