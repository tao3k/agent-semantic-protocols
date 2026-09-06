// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Server-owned content publication ledger.
//!
//! This is the V1 authority primitive. It is intentionally not connected to
//! the existing serving route until the content-binding proof gate passes.

use agent_semantic_content_identity::content_binding::{
    AuthorityStamp, ContentBinding, ContentBindingError, ContentIdentity, ContentPublicationCommit,
};

#[derive(Default)]
pub struct ContentPublicationLedger {
    active: Option<ContentPublicationCommit>,
}

impl ContentPublicationLedger {
    /// Accept only a schema-validated binding at the server publication point.
    /// The binding remains shadow/non-serving until the approved V1 cutover.
    pub fn publish_binding(
        &mut self,
        expected_digest: Option<&str>,
        binding: ContentBinding,
    ) -> Result<&ContentPublicationCommit, ContentBindingError> {
        binding.validate()?;
        self.publish(expected_digest, binding.identity, binding.authority_stamp)
    }

    pub fn publish(
        &mut self,
        expected_digest: Option<&str>,
        identity: ContentIdentity,
        authority_stamp: AuthorityStamp,
    ) -> Result<&ContentPublicationCommit, ContentBindingError> {
        if let Some(active) = self.active.as_ref() {
            if expected_digest != Some(active.commit_digest.as_str()) {
                return Err(ContentBindingError::ContentMismatch);
            }
        } else if expected_digest.is_some() {
            return Err(ContentBindingError::ContentMismatch);
        }

        let commit = ContentPublicationCommit::linearize_with_expected(
            identity,
            authority_stamp,
            expected_digest,
        )?;
        self.active = Some(commit);
        Ok(self.active.as_ref().expect("publication was just stored"))
    }

    pub fn admit_exact(
        &self,
        identity: &ContentIdentity,
    ) -> Result<&ContentPublicationCommit, ContentBindingError> {
        let active = self
            .active
            .as_ref()
            .ok_or(ContentBindingError::ContentMismatch)?;
        active.admit_exact(identity)?;
        Ok(active)
    }

    pub fn admit_binding(
        &self,
        binding: &ContentBinding,
    ) -> Result<&ContentPublicationCommit, ContentBindingError> {
        binding.validate()?;
        self.admit_exact(&binding.identity)
    }

    pub fn active(&self) -> Option<&ContentPublicationCommit> {
        self.active.as_ref()
    }

    pub fn clear_without_authority(&mut self) -> Result<(), ContentBindingError> {
        Err(ContentBindingError::RollbackRequiresAuthority)
    }
}
