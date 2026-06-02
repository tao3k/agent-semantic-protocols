use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use semantic_agent_hook::{
    HookRuntime, builtin_provider_manifests, parse_hook_activation, provider_manifest_digest,
};
use serde_json::json;

pub(super) fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("semantic-agent-hook-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}

pub(super) fn root_owned_rust_activation_json() -> String {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("digest manifest");
    serde_json::to_string_pretty(&json!({
        "schemaId": semantic_agent_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": semantic_agent_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": semantic_agent_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": semantic_agent_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": {"runtime": "semantic-agent-hook", "version": "test"},
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "providerCommandPrefix": [],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": ["src", "tests", "crates", "examples", "benches"],
                "configFiles": ["Cargo.toml", "Cargo.lock"],
                "sourceExtensions": [".rs"],
                "ignoredPathPrefixes": [".cache", ".direnv", ".git", ".idea", ".jj", ".run", ".vscode", "node_modules", "target", ".codex/harness-state", ".codex/rs-harness"]
            }
        }]
    }))
    .expect("serialize root-owned rust activation")
}

pub(super) fn write_root_owned_rust_activation(root: &std::path::Path) -> PathBuf {
    let path = root.join("rust-activation.json");
    std::fs::write(&path, root_owned_rust_activation_json()).expect("write rust activation");
    path
}

pub(super) fn write_fake_provider_binary(root: &std::path::Path, binary: &str) -> PathBuf {
    write_fake_provider_file(root, binary, 0o755)
}

pub(super) fn write_failing_provider_binary(root: &std::path::Path, binary: &str) -> PathBuf {
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake provider bin dir");
    let path = bin_dir.join(binary);
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf 'provider process should not be executed\\n' >&2\nexit 42\n",
    )
    .expect("write failing provider binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path)
            .expect("failing provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod failing provider");
    }
    bin_dir
}

pub(super) fn write_fake_provider_file(root: &std::path::Path, binary: &str, mode: u32) -> PathBuf {
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake provider bin dir");
    let path = bin_dir.join(binary);
    let guide_marker = match binary {
        "rs-harness" => {
            "[agent-guide] runtime=semantic-agent-hook language=rust provider=rs-harness"
        }
        "ts-harness" => "[ts-harness-guide]",
        "py-harness" => "[py-harness-guide]",
        _ => "[agent-guide]",
    };
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"agent\" ] && [ \"$2\" = \"guide\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 0\n",
            guide_marker
        ),
    )
    .expect("write fake provider binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&path)
            .expect("fake provider metadata")
            .permissions();
        permissions.set_mode(mode);
        std::fs::set_permissions(&path, permissions).expect("chmod fake provider");
    }
    bin_dir
}

pub(super) fn rust_harness_activation() -> HookRuntime {
    parse_hook_activation(&root_owned_rust_activation_json())
        .expect("valid root-owned rust activation")
}
