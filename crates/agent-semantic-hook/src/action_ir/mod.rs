// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Host facts and parser-derived semantic capabilities consumed by Hook rules.

mod capability;
mod host;
mod matching;
mod projection;
mod receipt;
mod subject;

pub(crate) use capability::FilesystemPermissionFact;
pub(crate) use capability::FilesystemPermissionKind;
pub(crate) use capability::FilesystemPermissionSource;
pub(crate) use capability::SemanticCapability;
pub(crate) use capability::SemanticCapabilityEvidence;
pub(crate) use host::AgentActionKind;
pub(crate) use host::HostInvocationFact;
pub(crate) use host::HostInvocationKind;
pub(crate) use matching::action_kind_matches;
pub(crate) use matching::host_invocation_kind_matches;
pub(crate) use projection::project_agent_action;
pub(crate) use receipt::AgentAction;
pub(crate) use subject::AgentActionSubject;
pub(crate) use subject::AgentActionSubjectKind;
