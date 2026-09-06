// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Runtime development-mode configuration contract tests.

use std::path::PathBuf;

use agent_semantic_config::runtime_dev::ArtifactOrigin;
use agent_semantic_config::runtime_dev::RuntimeArtifactMode;
use agent_semantic_config::runtime_dev::parse_runtime_artifact_mode;

#[test]
fn dev_model_accepts_only_development_workspace_artifacts() {
    let mode = parse_runtime_artifact_mode(
        "[dev]\nenabled = true\nroot = \"/checkout/agent-semantic-protocols\"\n",
    )
    .expect("valid dev model");
    assert_eq!(
        mode,
        RuntimeArtifactMode::Dev {
            root: PathBuf::from("/checkout/agent-semantic-protocols")
        }
    );
    assert!(mode.admits(ArtifactOrigin::DevelopWorkspace));
    assert!(!mode.admits(ArtifactOrigin::LockedRelease));
    assert!(!mode.admits(ArtifactOrigin::PathFallback));
}

#[test]
fn release_model_never_accepts_checkout_or_path_artifacts() {
    let mode = parse_runtime_artifact_mode("").expect("release model");
    assert_eq!(mode, RuntimeArtifactMode::Release);
    assert!(mode.admits(ArtifactOrigin::LockedRelease));
    assert!(!mode.admits(ArtifactOrigin::DevelopWorkspace));
    assert!(!mode.admits(ArtifactOrigin::PathFallback));
}

#[test]
fn enabled_dev_root_is_absolute_and_legacy_scope_is_rejected() {
    let relative =
        parse_runtime_artifact_mode("[dev]\nenabled = true\nroot = \"agent-semantic-protocols\"\n")
            .expect_err("relative dev root must fail closed");
    assert!(relative.contains("absolute path"));

    let legacy_scope = parse_runtime_artifact_mode(
        "[dev]\nenabled = true\nscope = \"checkout\"\nroot = \"/checkout\"\n",
    )
    .expect_err("legacy scope must not be accepted");
    assert!(legacy_scope.contains("unknown field `scope`"));

    let disabled = parse_runtime_artifact_mode("[dev]\nenabled = false\nroot = \"/checkout\"\n")
        .expect("disabled dev section selects release mode");
    assert_eq!(disabled, RuntimeArtifactMode::Release);

    let with_unrelated_state_config = parse_runtime_artifact_mode(
        "[runtime]\nresident = true\n\n[dev]\nenabled = true\nroot = \"/checkout\"\n",
    )
    .expect("state-home config may contain independently owned tables");
    assert_eq!(
        with_unrelated_state_config,
        RuntimeArtifactMode::Dev {
            root: PathBuf::from("/checkout")
        }
    );
}
