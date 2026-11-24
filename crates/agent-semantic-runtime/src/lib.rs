// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime state materialization for ASP project-local storage.

#![deny(dead_code)]

pub mod provider_catalog;
pub use provider_catalog::ProviderInstallReceipt;

mod async_bridge;
pub mod git;
mod live_corpus;
pub mod provider_workspace_artifact;
pub mod runtime_identity_monitor;
pub mod runtime_process_lifecycle;
mod runtime_source;
pub mod state;
pub mod state_core;
mod timeout_policy;

pub use async_bridge::runtime_block_on_current_thread;
pub use live_corpus::LIVE_CORPUS_ARTIFACT_SCHEMA_ID;
pub use live_corpus::LIVE_CORPUS_ARTIFACT_SCHEMA_VERSION;
pub use live_corpus::LiveCorpusArtifactGitIdentity;
pub use live_corpus::LiveCorpusArtifactIdentity;
pub use live_corpus::LiveCorpusArtifactManifest;
pub use live_corpus::LiveCorpusArtifactPaths;
pub use live_corpus::LiveCorpusGitCheckoutQualification;
pub use live_corpus::LiveCorpusGitCheckoutSync;
pub use live_corpus::LiveCorpusGitRepositoryPaths;
pub use live_corpus::LiveCorpusLanguageExtensionEvidence;
pub use live_corpus::live_corpus_artifact_manifest;
pub use live_corpus::live_corpus_artifact_paths;
pub use live_corpus::live_corpus_git_checkout_is_clean;
pub use live_corpus::live_corpus_git_repository_paths;
pub use live_corpus::live_corpus_lock_digest;
pub use live_corpus::qualify_live_corpus_embedded_document_extensions;
pub use live_corpus::qualify_live_corpus_git_checkout;
pub use live_corpus::qualify_live_corpus_language_extensions;
pub use live_corpus::sync_live_corpus_git_checkout;
pub use runtime_source::RuntimeSourceCheckout;
pub use runtime_source::RuntimeSourceIndexContext;
pub use runtime_source::RuntimeSourceIndexContextRequest;
pub use runtime_source::RuntimeSourceIndexFile;
pub use runtime_source::RuntimeSourceIndexFilesRequest;
pub use runtime_source::RuntimeSourceRegistryFingerprintRequest;
pub use runtime_source::RuntimeSourceSpec;
pub use runtime_source::collect_runtime_source_index_files;
pub use runtime_source::ensure_runtime_source_checkout;
pub use runtime_source::ensure_runtime_source_checkout_in_runtime_root;
pub use runtime_source::runtime_source_checkout_dir;
pub use runtime_source::runtime_source_checkout_dir_in_runtime_root;
pub use runtime_source::runtime_source_index_context;
pub use runtime_source::runtime_source_registry_fingerprint;
pub use state::ProjectRuntimeState;
pub use state::WorkspaceRuntimePaths;
pub use state::ensure_project_artifacts_dir;
pub use state::ensure_project_client_cache_dir;
pub use state::ensure_project_provider_bin_dir;
pub use state::ensure_project_provider_lock_dir;
pub use state::ensure_project_runtime_home;
pub use state::project_protocol_home_path;
pub use state::project_runtime_state;
pub use state::project_runtime_state_with_state_home;
pub use state::project_state_paths;
pub use state::project_state_paths_with_state_home;
pub use state::provider_package_dir;
pub use state::provider_receipt_dir;
pub use state::provider_state_root;
pub use state::temporary_workspace_runtime_state_with_owner_and_state_home;
pub use state_core::resolve_state_home;
pub use timeout_policy::RuntimeOperationTimeoutPolicy;
pub use timeout_policy::RuntimeOperationTimeoutReceipt;
pub use timeout_policy::runtime_operation_timeout_receipt;

#[cfg(test)]
#[path = "../tests/unit/runtime_host_authority.rs"]
mod runtime_host_authority_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_identity_monitor.rs"]
mod runtime_identity_monitor_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_process_lifecycle.rs"]
mod runtime_process_lifecycle_tests;
#[cfg(test)]
#[path = "../tests/unit/timeout_policy.rs"]
mod timeout_policy_tests;
