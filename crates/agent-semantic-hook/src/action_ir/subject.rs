// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentActionSubjectKind {
    RegisteredLanguageSource,
    RegisteredLanguageSourcePattern,
    Directory,
    StructuralSelector,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentActionSubject {
    pub(crate) value: String,
    pub(crate) kind: AgentActionSubjectKind,
}
