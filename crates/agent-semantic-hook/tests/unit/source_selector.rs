// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::project_shell_subject_paths;
use crate::HookPolicy;
use crate::HookProviderProjection;
use crate::HookRuntime;
use agent_semantic_config::LanguageId;
use agent_semantic_config::ProviderId;

#[test]
fn shell_subject_projection_fades_flags_and_non_file_operands() {
    let registry = HookRuntime {
        project_root: ".".to_string(),
        policy_providers: Vec::new(),
    };
    let paths = ["-n", "-xx", "self-apply-findings.ss", "1,10p"]
        .map(str::to_string)
        .to_vec();

    assert_eq!(
        project_shell_subject_paths(&registry, &paths),
        ["self-apply-findings.ss"]
    );
}

#[test]
fn shell_subject_projection_accepts_a_registered_source_root_descendant() {
    let registry = HookRuntime {
        project_root: ".".to_string(),
        policy_providers: vec![HookProviderProjection {
            language_id: LanguageId::new("rust"),
            provider_id: ProviderId::new("asp-rust"),
            package_roots: vec!["crates".to_owned()],
            source_extensions: vec![".rs".to_owned()],
            config_files: Vec::new(),
            policy: HookPolicy::default(),
        }],
    };

    assert_eq!(
        project_shell_subject_paths(&registry, &["crates/agent-semantic-runtime".to_owned()]),
        ["crates/agent-semantic-runtime"]
    );
}
