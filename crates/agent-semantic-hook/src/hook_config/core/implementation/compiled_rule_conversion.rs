// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_config::HookClientConfigReasonKind;
use agent_semantic_config::HookClientConfigStdinMode;

use crate::protocol::ReasonKind;
use crate::protocol::StdinMode;

impl From<HookClientConfigReasonKind> for ReasonKind {
    fn from(kind: HookClientConfigReasonKind) -> Self {
        match kind {
            HookClientConfigReasonKind::None => Self::None,
            HookClientConfigReasonKind::RegisteredSourceRouteRequired => {
                Self::RegisteredSourceRouteRequired
            }
            HookClientConfigReasonKind::StructuredSourceRead => Self::StructuredSourceRead,
            HookClientConfigReasonKind::BulkSourceDump => Self::BulkSourceDump,
            HookClientConfigReasonKind::RawBroadSearch => Self::RawBroadSearch,
            HookClientConfigReasonKind::AgentSearchJson => Self::AgentSearchJson,
            HookClientConfigReasonKind::AgentChoiceRequired => Self::AgentChoiceRequired,
        }
    }
}

impl From<HookClientConfigStdinMode> for StdinMode {
    fn from(mode: HookClientConfigStdinMode) -> Self {
        match mode {
            HookClientConfigStdinMode::None => Self::None,
            HookClientConfigStdinMode::PipeCandidates => Self::PipeCandidates,
            HookClientConfigStdinMode::PipeDiff => Self::PipeDiff,
            HookClientConfigStdinMode::Unknown => Self::Unknown,
        }
    }
}
