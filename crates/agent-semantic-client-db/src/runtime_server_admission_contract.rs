// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Stable v1 ownership vocabulary for Runtime-owned workspace generation admission.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationAdmissionTrigger {
    QueryDemand,
    WorkspaceChange,
    OperatorMutation,
    ArtifactPublication,
    RuntimeRecovery,
}

impl WorkspaceGenerationAdmissionTrigger {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::QueryDemand => "query-demand",
            Self::WorkspaceChange => "workspace-change",
            Self::OperatorMutation => "operator-mutation",
            Self::ArtifactPublication => "artifact-publication",
            Self::RuntimeRecovery => "runtime-recovery",
        }
    }
}

impl std::fmt::Display for WorkspaceGenerationAdmissionTrigger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationAdmissionMode {
    CompleteGeneration,
    FullRecovery,
}

impl WorkspaceGenerationAdmissionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompleteGeneration => "complete-generation",
            Self::FullRecovery => "full-recovery",
        }
    }

    pub const fn is_complete_generation(self) -> bool {
        matches!(self, Self::CompleteGeneration | Self::FullRecovery)
    }
}

impl std::fmt::Display for WorkspaceGenerationAdmissionMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
