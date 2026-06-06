use agent_semantic_hook::{builtin_provider_manifests, provider_manifest_digest};
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

pub(super) struct ProviderSpec {
    language_id: &'static str,
    command_prefix: Vec<String>,
}

pub(super) fn provider(language_id: &'static str, command_prefix: Vec<String>) -> ProviderSpec {
    ProviderSpec {
        language_id,
        command_prefix,
    }
}

pub(super) fn write_activation(root: &Path, providers: &[ProviderSpec]) {
    let activation_dir = root.join(".cache/agent-semantic-protocol/hooks");
    std::fs::create_dir_all(&activation_dir).expect("create activation dir");
    let providers: Vec<_> = providers
        .iter()
        .map(|spec| {
            let manifest = builtin_provider_manifests()
                .into_iter()
                .find(|manifest| manifest.language_id == spec.language_id)
                .unwrap_or_else(|| panic!("missing manifest for {}", spec.language_id));
            let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
            json!({
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
            })
        })
        .collect();
    let activation = json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": { "runtime": "asp", "version": "test" },
        "providers": providers
    });
    std::fs::write(
        activation_dir.join("activation.json"),
        serde_json::to_string_pretty(&activation).expect("serialize activation"),
    )
    .expect("write activation");
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

pub(super) fn cache_root(root: &Path) -> PathBuf {
    root.join(".cache/agent-semantic-protocol/client")
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

pub(super) fn asp_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.current_dir(root).env_remove("PRJ_CACHE_HOME");
    command
}

pub(super) fn prepend_path(path_prefix: &Path) -> OsString {
    let mut paths = vec![path_prefix.to_path_buf()];
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    env::join_paths(paths).expect("join PATH")
}

pub(super) fn temp_project_root(name: &str) -> PathBuf {
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

pub(super) fn write_marker_provider(bin_dir: &Path, binary: &str, marker: &Path) {
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
        "#!/bin/sh\nprintf '[agent-guide] language=rust provider=rs-harness\\n'\nprintf '|cmd prime=rs-harness search prime .\\n'\nprintf \"|cmd ingest=rg -n '<query>' src tests | rs-harness search ingest .\\n\"\nprintf '|cmd ast-patch=rs-harness ast-patch dry-run --packet <semantic-ast-patch.json|-> .\\n'\nprintf '|cmd evidence=rs-harness evidence graph --review-packet-json <path> --json .\\n'\nprintf '|rule hook install/runtime is owned by agent-semantic-hook\\n'\n",
    );
}

pub(super) fn write_command_hint_provider(bin_dir: &Path, binary: &str) {
    write_provider_script(
        bin_dir,
        binary,
        "#!/bin/sh\nprintf '{\"provider\":\"rs-harness\",\"next\":\"rs-harness query src/lib.rs .\"}\\n'\n",
    );
}

pub(super) fn write_stdin_provider(bin_dir: &Path, binary: &str) {
    write_provider_script(
        bin_dir,
        binary,
        "#!/bin/sh\nIFS= read -r line\nprintf 'stdin=%s\\n' \"$line\"\n",
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
