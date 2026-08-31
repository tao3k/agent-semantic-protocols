use serde::{Deserialize, Serialize};

use crate::content_binding::{
    CONTENT_BINDING_SCHEMA_VERSION, ContentBinding, ContentBindingError, ContentPublicationCommit,
};
use crate::runtime_execution::{RuntimeExecutionBinding, RuntimeExecutionBindingError};

pub const SEARCH_EXECUTION_SCHEMA_ID: &str = "asp.search-execution";
pub const SEARCH_EXECUTION_SCHEMA_VERSION: &str = CONTENT_BINDING_SCHEMA_VERSION;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchOperation {
    Prime,
    QuerySet,
    Search,
    Query,
    Exact,
    Pipe,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchExecutionContext {
    pub binding: ContentBinding,
    pub commit_digest: String,
    #[serde(default)]
    pub runtime_binding: Option<RuntimeExecutionBinding>,
}

impl SearchExecutionContext {
    pub fn exact(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
    ) -> Result<Self, SearchExecutionError> {
        commit
            .admit_exact(&binding.identity)
            .map_err(SearchExecutionError::Binding)?;
        if commit.commit_digest != binding.identity.digest() {
            return Err(SearchExecutionError::CommitDigestMismatch);
        }
        Ok(Self {
            binding,
            commit_digest: commit.commit_digest.clone(),
            runtime_binding: None,
        })
    }

    pub fn exact_with_runtime_binding(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
        runtime_binding: RuntimeExecutionBinding,
    ) -> Result<Self, SearchExecutionError> {
        runtime_binding
            .validate()
            .map_err(SearchExecutionError::RuntimeBinding)?;
        let mut context = Self::exact(binding, commit)?;
        context.runtime_binding = Some(runtime_binding);
        Ok(context)
    }

    pub fn validate_against(
        &self,
        commit: &ContentPublicationCommit,
    ) -> Result<(), SearchExecutionError> {
        self.binding
            .validate()
            .map_err(SearchExecutionError::Binding)?;
        commit
            .admit_exact(&self.binding.identity)
            .map_err(SearchExecutionError::Binding)?;
        if self.commit_digest != commit.commit_digest
            || self.commit_digest != self.binding.identity.digest()
        {
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchClientFrame {
    pub frame_schema_id: String,
    pub frame_schema_version: String,
    pub request_id: String,
    pub session_id: String,
    pub operation: SearchOperation,
    pub context: SearchExecutionContext,
    pub selector: Option<String>,
    pub cancellation_id: String,
}

impl SearchClientFrame {
    pub fn validate(&self, commit: &ContentPublicationCommit) -> Result<(), SearchExecutionError> {
        if self.frame_schema_id != SEARCH_EXECUTION_SCHEMA_ID
            || self.frame_schema_version != SEARCH_EXECUTION_SCHEMA_VERSION
        {
            return Err(SearchExecutionError::SchemaMismatch);
        }
        if self.request_id.is_empty()
            || self.session_id.is_empty()
            || self.cancellation_id.is_empty()
        {
            return Err(SearchExecutionError::MissingFrameIdentity);
        }
        self.context.validate_against(commit)?;
        if matches!(
            self.operation,
            SearchOperation::Query | SearchOperation::Exact | SearchOperation::Pipe
        ) && self.selector.as_deref().map(str::is_empty).unwrap_or(true)
        {
            return Err(SearchExecutionError::StaleSelector);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalReceipt {
    pub request_id: String,
    pub commit_digest: String,
    pub status: TerminalStatus,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SearchExecutionError {
    Binding(ContentBindingError),
    RuntimeBinding(RuntimeExecutionBindingError),
    CommitDigestMismatch,
    SchemaMismatch,
    MissingFrameIdentity,
    StaleSelector,
    AlreadyTerminal,
    Cancelled,
}

pub struct SearchExecution {
    context: SearchExecutionContext,
    terminal: Option<TerminalReceipt>,
}

impl SearchExecution {
    pub fn prime(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
    ) -> Result<Self, SearchExecutionError> {
        Ok(Self {
            context: SearchExecutionContext::exact(binding, commit)?,
            terminal: None,
        })
    }

    pub fn prime_with_runtime_binding(
        binding: ContentBinding,
        commit: &ContentPublicationCommit,
        runtime_binding: RuntimeExecutionBinding,
    ) -> Result<Self, SearchExecutionError> {
        Ok(Self {
            context: SearchExecutionContext::exact_with_runtime_binding(
                binding,
                commit,
                runtime_binding,
            )?,
            terminal: None,
        })
    }

    pub fn context(&self) -> &SearchExecutionContext {
        &self.context
    }

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

    pub fn finish(
        &mut self,
        request_id: String,
        status: TerminalStatus,
    ) -> Result<&TerminalReceipt, SearchExecutionError> {
        if self.terminal.is_some() {
            return Err(SearchExecutionError::AlreadyTerminal);
        }
        if request_id.is_empty() {
            return Err(SearchExecutionError::MissingFrameIdentity);
        }
        self.terminal = Some(TerminalReceipt {
            request_id,
            commit_digest: self.context.commit_digest.clone(),
            status,
        });
        Ok(self.terminal.as_ref().expect("terminal was just inserted"))
    }

    pub fn cancel(&mut self, request_id: String) -> Result<&TerminalReceipt, SearchExecutionError> {
        self.finish(request_id, TerminalStatus::Cancelled)
    }

    pub fn terminal(&self) -> Option<&TerminalReceipt> {
        self.terminal.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_binding::{AuthorityStamp, ContentIdentity};

    fn identity() -> ContentIdentity {
        let digest = "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        ContentIdentity {
            runtime_artifact_digest: digest.into(),
            workspace_snapshot_digest: digest.into(),
            source_generation_digest: digest.into(),
            source_index_digest: digest.into(),
            schema_digest: digest.into(),
            provider_catalog_digest: digest.into(),
        }
    }

    fn commit() -> ContentPublicationCommit {
        let identity = identity();
        let digest = identity.digest();
        ContentPublicationCommit::linearize(
            identity,
            AuthorityStamp {
                key_id: "test-key".into(),
                canonical_digest: digest,
                signature: "test-signature".into(),
            },
        )
        .expect("valid commit")
    }

    #[test]
    fn prime_pins_one_exact_context_for_all_operations() {
        let commit = commit();
        let execution = SearchExecution::prime(
            ContentBinding::new(
                commit.identity.clone(),
                AuthorityStamp {
                    key_id: "test-key".into(),
                    canonical_digest: commit.commit_digest.clone(),
                    signature: "test-signature".into(),
                },
            )
            .expect("valid binding"),
            &commit,
        )
        .expect("prime");
        let frame = SearchClientFrame {
            frame_schema_id: SEARCH_EXECUTION_SCHEMA_ID.into(),
            frame_schema_version: SEARCH_EXECUTION_SCHEMA_VERSION.into(),
            request_id: "request-1".into(),
            session_id: "session-1".into(),
            operation: SearchOperation::Query,
            context: execution.context().clone(),
            selector: Some("rust://item/1".into()),
            cancellation_id: "cancel-1".into(),
        };
        execution
            .admit(&frame, &commit)
            .expect("same content admits");
    }

    #[test]
    fn cancellation_is_terminal_and_cannot_be_followed_by_success() {
        let commit = commit();
        let mut execution = SearchExecution::prime(
            ContentBinding::new(
                commit.identity.clone(),
                AuthorityStamp {
                    key_id: "test-key".into(),
                    canonical_digest: commit.commit_digest.clone(),
                    signature: "test-signature".into(),
                },
            )
            .expect("valid binding"),
            &commit,
        )
        .expect("prime");
        execution.cancel("request-1".into()).expect("cancel");
        assert_eq!(
            execution.finish("request-1".into(), TerminalStatus::Succeeded),
            Err(SearchExecutionError::AlreadyTerminal)
        );
        assert_eq!(
            execution.terminal().map(|receipt| receipt.status),
            Some(TerminalStatus::Cancelled)
        );
    }
}
