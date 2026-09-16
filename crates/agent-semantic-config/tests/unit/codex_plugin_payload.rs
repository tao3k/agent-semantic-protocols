// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::Path;

use super::CODEX_PLUGIN_HOOKS_RELATIVE_PATH;
use super::CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH;
use super::CODEX_PLUGIN_MANIFEST_RELATIVE_PATH;
use super::load_codex_plugin_payload_identity;

const MANIFEST: &[u8] = include_bytes!("../../../../asp-codex-plugin/.codex-plugin/plugin.json");
const HOOKS: &[u8] = include_bytes!("../../../../asp-codex-plugin/hooks/hooks.json");
const LAUNCHER: &[u8] = include_bytes!("../../../../asp-codex-plugin/bin/asp-hook-exec");

#[test]
fn canonical_payload_has_a_typed_content_identity() {
    let fixture = tempfile::tempdir().expect("payload fixture");
    write_bundle(fixture.path(), MANIFEST, HOOKS, LAUNCHER);
    let identity = load_codex_plugin_payload_identity(fixture.path()).expect("identity");
    assert_eq!(identity.plugin_name, "asp-codex-plugin");
    assert!(identity.digest.starts_with("blake3-256:"));
}

#[test]
fn disk_validator_rejects_client_binary_as_hook_authority() {
    let fixture = tempfile::tempdir().expect("payload fixture");
    let launcher = String::from_utf8(LAUNCHER.to_vec())
        .expect("launcher UTF-8")
        .replace(
            "runtime/artifacts/active/asp-hook",
            "runtime/artifacts/active/asp",
        );
    write_bundle(fixture.path(), MANIFEST, HOOKS, launcher.as_bytes());
    assert!(
        load_codex_plugin_payload_identity(fixture.path())
            .expect_err("Client binary Hook authority must fail")
            .contains("canonical Runtime Hook binary")
    );
}

fn write_bundle(root: &Path, manifest: &[u8], hooks: &[u8], launcher: &[u8]) {
    for (relative, bytes) in [
        (CODEX_PLUGIN_MANIFEST_RELATIVE_PATH, manifest),
        (CODEX_PLUGIN_HOOKS_RELATIVE_PATH, hooks),
        (CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH, launcher),
    ] {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("payload parent")).expect("create payload parent");
        fs::write(path, bytes).expect("write payload");
    }
}
