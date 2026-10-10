// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! V1 contract shared by the bounded Hook writer and Runtime reconciler.

use serde::{Deserialize, Serialize};

/// Canonical V1 Hook memory inbox schema identity.
pub const HOOK_MEMORY_INBOX_SCHEMA_ID: &str = "agent.semantic-protocols.hook.memory-inbox";
/// Canonical V1 typed failure schema identity.
pub const HOOK_MEMORY_INBOX_FAILURE_SCHEMA_ID: &str =
    "agent.semantic-protocols.hook.memory-inbox-failure";
/// Stable schema version used by both inbox contracts.
pub const HOOK_MEMORY_INBOX_SCHEMA_VERSION: &str = "1";

/// Semantic class of one inbox record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookMemoryInboxEntryKind {
    HostLifecycle,
    HostExecutionObservation,
    WorkspaceMutation,
}

/// Exact changed-owner cut emitted by a successful Host mutation tool.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookWorkspaceMutationEvent {
    /// Stable Host delivery identity used for replay coalescing.
    pub mutation_id: String,
    /// Canonical absolute worktree root; Runtime binds its admitted identity.
    pub project_root: String,
    /// Strictly ordered, unique workspace-relative changed owner paths.
    pub changed_paths: Vec<String>,
    /// Host tool whose parser produced the changed owner cut.
    pub tool_name: String,
    /// Optional Host session evidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Optional Host tool-use evidence and preferred mutation identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

impl HookWorkspaceMutationEvent {
    /// Validates the normalized Host-to-Runtime mutation boundary.
    pub fn validate(&self) -> Result<(), String> {
        if self.mutation_id.is_empty() {
            return Err("Hook workspace mutation id must not be empty".to_owned());
        }
        if self.project_root.is_empty() || !std::path::Path::new(&self.project_root).is_absolute() {
            return Err("Hook workspace mutation projectRoot must be absolute".to_owned());
        }
        if self.tool_name.is_empty() {
            return Err("Hook workspace mutation toolName must not be empty".to_owned());
        }
        if self.changed_paths.is_empty() {
            return Err("Hook workspace mutation changedPaths must not be empty".to_owned());
        }
        let mut previous = None;
        for path in &self.changed_paths {
            let valid = !path.is_empty()
                && !path.starts_with('/')
                && !path.contains('\\')
                && path
                    .split('/')
                    .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
            if !valid {
                return Err(format!(
                    "invalid Hook workspace-relative changed path: {path}"
                ));
            }
            if previous.is_some_and(|previous| previous >= path.as_str()) {
                return Err("Hook changedPaths must be strictly ordered and unique".to_owned());
            }
            previous = Some(path.as_str());
        }
        Ok(())
    }
}

/// Typed event payload carried inside the generic V1 inbox envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum HookMemoryInboxEventPayload {
    /// Exact workspace mutation event.
    WorkspaceMutation(HookWorkspaceMutationEvent),
}

/// Checksummed, monotonically sequenced V1 inbox record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookMemoryInboxEvent {
    /// Canonical contract identity.
    pub schema_id: String,
    /// Stable V1 version.
    pub schema_version: String,
    /// Globally monotone inbox sequence.
    pub inbox_sequence: u64,
    /// Semantic record class.
    pub entry_kind: HookMemoryInboxEntryKind,
    /// Typed record payload.
    pub event: HookMemoryInboxEventPayload,
}

impl HookMemoryInboxEvent {
    /// Constructs one validated workspace mutation record.
    pub fn workspace_mutation(
        inbox_sequence: u64,
        event: HookWorkspaceMutationEvent,
    ) -> Result<Self, String> {
        event.validate()?;
        if inbox_sequence == 0 {
            return Err("Hook memory inbox sequence must be positive".to_owned());
        }
        Ok(Self {
            schema_id: HOOK_MEMORY_INBOX_SCHEMA_ID.to_owned(),
            schema_version: HOOK_MEMORY_INBOX_SCHEMA_VERSION.to_owned(),
            inbox_sequence,
            entry_kind: HookMemoryInboxEntryKind::WorkspaceMutation,
            event: HookMemoryInboxEventPayload::WorkspaceMutation(event),
        })
    }

    /// Validates schema identity, sequence, kind, and payload invariants.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != HOOK_MEMORY_INBOX_SCHEMA_ID
            || self.schema_version != HOOK_MEMORY_INBOX_SCHEMA_VERSION
        {
            return Err("Hook memory inbox schema identity mismatch".to_owned());
        }
        if self.inbox_sequence == 0 {
            return Err("Hook memory inbox sequence must be positive".to_owned());
        }
        if self.entry_kind == HookMemoryInboxEntryKind::WorkspaceMutation {
            let HookMemoryInboxEventPayload::WorkspaceMutation(event) = &self.event;
            event.validate()?;
        }
        Ok(())
    }

    /// Projects the typed workspace mutation carried by this record.
    pub fn workspace_mutation_event(&self) -> Result<Option<HookWorkspaceMutationEvent>, String> {
        self.validate()?;
        if self.entry_kind != HookMemoryInboxEntryKind::WorkspaceMutation {
            return Ok(None);
        }
        let HookMemoryInboxEventPayload::WorkspaceMutation(event) = &self.event;
        Ok(Some(event.clone()))
    }
}

/// Stable reason taxonomy for a Hook-local inbox failure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookMemoryInboxFailureReason {
    CapacityExhausted,
    ChecksumMismatch,
    CompactEncode,
    DecodeRecord,
    EncodeEvent,
    EncodeObservation,
    EncodePerformanceObservation,
    EncodeRecord,
    FlushSchedule,
    HeaderDecode,
    HeaderRange,
    LengthDecode,
    LockBudgetExceeded,
    LockFailed,
    LockOpen,
    MagicMismatch,
    Mmap,
    Open,
    Preallocate,
    RecordSizeOverflow,
    RecordTooLarge,
    SequenceOverflow,
    SizeMismatch,
    Stat,
    TruncatedHeader,
    TruncatedRecord,
    Unlock,
    WriteOffsetInvalid,
}

/// Typed terminal returned when the Hook cannot publish one local record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookMemoryInboxFailure {
    /// Canonical failure schema identity.
    pub schema_id: String,
    /// Stable V1 version.
    pub schema_version: String,
    /// Terminal failure state.
    pub state: String,
    /// Machine-readable failure reason.
    pub reason_kind: HookMemoryInboxFailureReason,
    /// Human-readable diagnostic.
    pub detail: String,
    /// V1 never hides a retry loop inside the Hook.
    pub retry_after_ms: u64,
}

impl HookMemoryInboxFailure {
    /// Constructs one typed terminal failure.
    pub fn new(reason_kind: HookMemoryInboxFailureReason, detail: impl Into<String>) -> Self {
        Self {
            schema_id: HOOK_MEMORY_INBOX_FAILURE_SCHEMA_ID.to_owned(),
            schema_version: HOOK_MEMORY_INBOX_SCHEMA_VERSION.to_owned(),
            state: "failed".to_owned(),
            reason_kind,
            detail: detail.into(),
            retry_after_ms: 0,
        }
    }
}
