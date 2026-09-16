// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::PROVIDER_WORKSPACE_INSTALL_SCHEMA_AUTHORITY;
use crate::PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID;
use crate::PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION;
use crate::ProviderWorkspaceInstallDescriptor;
use crate::workspace_install::validate_environment_removals;

#[test]
fn environment_prefix_cannot_be_empty() {
    let error = validate_environment_removals("workspaceBuild", &[], &[String::new()])
        .expect_err("empty prefix must fail closed");
    assert!(error.contains("invalid environment identity"), "{error}");
}

#[test]
fn schema_reference_may_follow_the_receipted_bundle_root() {
    for schema in [
        "../schemas/provider-workspace-install.schema.json",
        "org/schemas/provider-workspace-install.schema.json",
    ] {
        let descriptor: ProviderWorkspaceInstallDescriptor =
            serde_json::from_value(serde_json::json!({
                "$schema": schema,
                "schemaId": PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID,
                "schemaVersion": PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION,
                "schemaAuthority": PROVIDER_WORKSPACE_INSTALL_SCHEMA_AUTHORITY,
                "languageId": "fixture",
                "providerId": "asp-fixture",
                "binary": "asp-fixture",
                "providerRegistration": "asp-provider-registration.json",
                "schemaBundleReceipt": "schemas/.asp-schema-manager-receipt.json",
                "workspaceArtifact": {"root": "build/provider", "entrypoint": "bin/asp-fixture"},
                "workspaceBuild": {
                    "program": "cargo",
                    "args": ["build"],
                    "workingDirectory": ".",
                    "sourceSnapshotAnchors": ["Cargo.toml"],
                    "derivedPaths": ["build/provider"],
                    "env": {}
                }
            }))
            .expect("descriptor");
        descriptor.validate().expect("bundle-relative schema");
    }
}
