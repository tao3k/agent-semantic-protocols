// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Exact-content execution identity for every public search operation.

use serde::Deserialize;
use serde::Serialize;

use crate::ProjectWorkspaceBinding;
use crate::content_binding::CONTENT_BINDING_SCHEMA_VERSION;
use crate::content_binding::ContentBinding;
use crate::content_binding::ContentBindingError;
use crate::content_binding::ContentPublicationCommit;
use crate::runtime_execution::RuntimeExecutionBinding;
use crate::runtime_execution::RuntimeExecutionBindingError;

/// Stable schema identifier for search execution frames.
pub const SEARCH_EXECUTION_SCHEMA_ID: &str = "asp.search-execution";
/// Stable schema version shared with the content-binding contract.
pub const SEARCH_EXECUTION_SCHEMA_VERSION: &str = CONTENT_BINDING_SCHEMA_VERSION;

/// Public search operation admitted by an exact content binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchOperation {
    /// Execute the complete search playbook.
    Playbook,
    /// Evaluate one search query.
    Query,
    /// Resolve one exact structural selector.
    Exact,
}

/// Exact content and optional Runtime identity bound to one execution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchExecutionContext {
    /// Content identity admitted for this execution.
    pub binding: ContentBinding,
    /// Digest of the publication commit that admitted `binding`.
    pub commit_digest: String,
    /// Runtime and evaluator identity when execution crosses the Runtime boundary.
    #[serde(default)]
    pub runtime_binding: Option<RuntimeExecutionBinding>,
}

impl SearchExecutionContext {
    /// Constructs a context pinned to one exact content publication.
    pub fn exact(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
    ) -> Result<Self, SearchExecutionError> {
        commit
            .admit_exact(&binding)
            .map_err(SearchExecutionError::Binding)?;
        Ok(Self {
            binding,
            commit_digest: commit.commit_digest.clone(),
            runtime_binding: None,
        })
    }

    /// Constructs a context pinned to content and Runtime identities.
    pub fn exact_with_runtime_binding(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
        runtime_binding: RuntimeExecutionBinding,
        manifest_project_workspace: &ProjectWorkspaceBinding,
    ) -> Result<Self, SearchExecutionError> {
        runtime_binding
            .validate()
            .map_err(SearchExecutionError::RuntimeBinding)?;
        manifest_project_workspace
            .validate()
            .map_err(|_| SearchExecutionError::ProjectWorkspaceManifestMismatch)?;
        if runtime_binding.project_workspace != *manifest_project_workspace {
            return Err(SearchExecutionError::ProjectWorkspaceManifestMismatch);
        }
        let mut context = Self::exact(binding, commit)?;
        context.runtime_binding = Some(runtime_binding);
        Ok(context)
    }

    /// Revalidates this context against the currently supplied publication.
    pub fn validate_against(
        &self,
        commit: &ContentPublicationCommit,
    ) -> Result<(), SearchExecutionError> {
        self.binding
            .validate()
            .map_err(SearchExecutionError::Binding)?;
        commit
            .admit_exact(&self.binding)
            .map_err(SearchExecutionError::Binding)?;
        if self.commit_digest != commit.commit_digest {
            return Err(SearchExecutionError::CommitDigestMismatch);
        }
        if let Some(runtime_binding) = &self.runtime_binding {
            runtime_binding
                .validate()
                .map_err(SearchExecutionError::RuntimeBinding)?;
        }
        Ok(())
    }
}

macro_rules! search_identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Borrows the wire identity value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

search_identity!(SearchRequestId, "Identity of one admitted search request.");
search_identity!(
    SearchSessionId,
    "Identity of the session that owns a search request."
);
search_identity!(
    SearchCancellationId,
    "Identity of the cancellation channel bound to a search request."
);

/// Raw search client-frame DTO whose identity fields use typed wire values.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchClientFrame {
    /// Schema identifier carried by the client frame.
    pub frame_schema_id: String,
    /// Schema version carried by the client frame.
    pub frame_schema_version: String,
    /// Request identity.
    pub request_id: SearchRequestId,
    /// Owning session identity.
    pub session_id: SearchSessionId,
    /// Search operation.
    pub operation: SearchOperation,
    /// Exact execution context.
    pub context: SearchExecutionContext,
    /// Optional provider-native exact selector.
    pub selector: Option<String>,
    /// Cancellation identity.
    pub cancellation_id: SearchCancellationId,
}

impl SearchClientFrame {
    /// Validates schema, identity, selector, and content binding.
    pub fn validate(&self, commit: &ContentPublicationCommit) -> Result<(), SearchExecutionError> {
        if self.frame_schema_id != SEARCH_EXECUTION_SCHEMA_ID
            || self.frame_schema_version != SEARCH_EXECUTION_SCHEMA_VERSION
        {
            return Err(SearchExecutionError::SchemaMismatch);
        }
        if self.request_id.as_str().is_empty()
            || self.session_id.as_str().is_empty()
            || self.cancellation_id.as_str().is_empty()
        {
            return Err(SearchExecutionError::MissingFrameIdentity);
        }
        self.context.validate_against(commit)?;
        if matches!(
            self.operation,
            SearchOperation::Query | SearchOperation::Exact
        ) && self.selector.as_deref().map(str::is_empty).unwrap_or(true)
        {
            return Err(SearchExecutionError::StaleSelector);
        }
        Ok(())
    }
}

/// Terminal state of an admitted search request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalStatus {
    /// Search completed successfully.
    Succeeded,
    /// Search completed with a typed failure.
    Failed,
    /// Search was cancelled.
    Cancelled,
}

/// Durable terminal receipt for one search request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalReceipt {
    /// Request identity.
    pub request_id: SearchRequestId,
    /// Exact content commit digest used by the request.
    pub commit_digest: String,
    /// Terminal status.
    pub status: TerminalStatus,
}

/// Typed failure returned by search execution admission and completion.
#[derive(Debug, Eq, PartialEq)]
pub enum SearchExecutionError {
    /// Content binding is invalid.
    Binding(ContentBindingError),
    /// Runtime binding is invalid.
    RuntimeBinding(RuntimeExecutionBindingError),
    /// Runtime binding Project Workspace differs from the parser-owned manifest.
    ProjectWorkspaceManifestMismatch,
    /// Frame and publication commit digests differ.
    CommitDigestMismatch,
    /// Frame schema identity is not current.
    SchemaMismatch,
    /// A required request, session, or cancellation identity is empty.
    MissingFrameIdentity,
    /// A selector-required operation omitted its selector.
    StaleSelector,
    /// The execution already emitted a terminal receipt.
    AlreadyTerminal,
    /// The execution was cancelled.
    Cancelled,
}

/// One exact-content search execution with a single terminal transition.
pub struct SearchExecution {
    context: SearchExecutionContext,
    terminal: Option<TerminalReceipt>,
}

impl SearchExecution {
    /// Binds a new execution to one exact content publication.
    pub fn bind(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
    ) -> Result<Self, SearchExecutionError> {
        Ok(Self {
            context: SearchExecutionContext::exact(binding, commit)?,
            terminal: None,
        })
    }

    /// Binds a new execution to content and Runtime identities.
    pub fn bind_with_runtime_binding(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
        runtime_binding: RuntimeExecutionBinding,
        manifest_project_workspace: &ProjectWorkspaceBinding,
    ) -> Result<Self, SearchExecutionError> {
        Ok(Self {
            context: SearchExecutionContext::exact_with_runtime_binding(
                binding,
                commit,
                runtime_binding,
                manifest_project_workspace,
            )?,
            terminal: None,
        })
    }

    /// Returns the exact execution context.
    pub fn context(&self) -> &SearchExecutionContext {
        &self.context
    }

    /// Admits a client frame only when all identities match this execution.
    pub fn admit(
        &self,
        frame: &SearchClientFrame,
        commit: &ContentPublicationCommit,
    ) -> Result<(), SearchExecutionError> {
        if self.terminal.is_some() {
            return Err(SearchExecutionError::AlreadyTerminal);
        }
        frame.validate(commit)?;
        if frame.context != self.context {
            return Err(SearchExecutionError::CommitDigestMismatch);
        }
        Ok(())
    }

    /// Emits the single terminal receipt for this execution.
    pub fn finish(
        &mut self,
        request_id: SearchRequestId,
        status: TerminalStatus,
    ) -> Result<&TerminalReceipt, SearchExecutionError> {
        if self.terminal.is_some() {
            return Err(SearchExecutionError::AlreadyTerminal);
        }
        if request_id.as_str().is_empty() {
            return Err(SearchExecutionError::MissingFrameIdentity);
        }
        self.terminal = Some(TerminalReceipt {
            request_id,
            commit_digest: self.context.commit_digest.clone(),
            status,
        });
        Ok(self.terminal.as_ref().expect("terminal was just inserted"))
    }

    /// Emits the cancellation terminal for this execution.
    pub fn cancel(
        &mut self,
        request_id: SearchRequestId,
    ) -> Result<&TerminalReceipt, SearchExecutionError> {
        self.finish(request_id, TerminalStatus::Cancelled)
    }

    /// Returns the terminal receipt when the execution has completed.
    pub fn terminal(&self) -> Option<&TerminalReceipt> {
        self.terminal.as_ref()
    }
}

#[cfg(test)]
#[path = "../tests/unit/search_execution.rs"]
mod tests;
