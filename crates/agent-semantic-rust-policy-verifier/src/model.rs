use std::path::Path;

use agent_semantic_rust_policy_types::SourceSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationInput<'a> {
    pub package_name: &'a str,
    pub manifest_dir: &'a Path,
    pub workspace_root: &'a Path,
    pub observed_source_snapshot: &'a SourceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedMemberReceipt {
    pub package_name: String,
    pub package_directory: String,
    pub gate_label: String,
    pub source_snapshot: SourceSnapshot,
    pub policy_digest: String,
    pub execution_command_digest: String,
    pub cache_payload_digest: String,
}
