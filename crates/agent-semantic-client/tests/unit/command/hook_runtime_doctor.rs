use super::{CodexMultiAgentV2Status, codex_multi_agent_v2_status};

#[test]
fn doctor_requires_codex_multi_agent_v2_true() {
    assert_eq!(
        codex_multi_agent_v2_status(Some("[features]\nmulti_agent_v2 = true\n")),
        CodexMultiAgentV2Status::Enabled
    );
    assert_eq!(
        codex_multi_agent_v2_status(Some("[features]\nmulti_agent_v2 = false\n")),
        CodexMultiAgentV2Status::FeatureDisabled
    );
    assert_eq!(
        codex_multi_agent_v2_status(Some("[features]\nother = true\n")),
        CodexMultiAgentV2Status::FeatureMissing
    );
    assert_eq!(
        codex_multi_agent_v2_status(Some("multi_agent_v2 = true\n")),
        CodexMultiAgentV2Status::FeatureMissing
    );
    assert_eq!(
        codex_multi_agent_v2_status(Some("[features\n")),
        CodexMultiAgentV2Status::ConfigInvalid
    );
    assert_eq!(
        codex_multi_agent_v2_status(None),
        CodexMultiAgentV2Status::ConfigMissing
    );
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
