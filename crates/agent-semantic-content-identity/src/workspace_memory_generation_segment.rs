/// Binary contract discriminator for an immutable resident generation that
/// includes the provider relation graph in its content identity.
pub const WORKSPACE_MEMORY_GENERATION_SEGMENT_MAGIC: &[u8; 16] = b"ASPWSMEMORYRELV1";

pub const WORKSPACE_MEMORY_GENERATION_SEGMENT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-memory-generation-segment";

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceMemoryGenerationSectionKindV1 {
    GenerationEvidence,
    ProjectResolutions,
    OwnerDirectory,
    OwnerBytes,
    LexicalIndex,
    SelectorIndex,
    GraphRelations,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceMemoryGenerationSectionRepresentationV1 {
    FixedLeU64,
    SortedOffsetTable,
    Utf8StringTable,
    OpaqueOwnerBytes,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceMemoryGenerationSectionV1 {
    pub kind: WorkspaceMemoryGenerationSectionKindV1,
    pub offset: u64,
    pub length: u64,
    pub record_count: u64,
    pub representation: WorkspaceMemoryGenerationSectionRepresentationV1,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceMemoryGenerationDirectoryV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub active_epoch: u64,
    pub root_depth: [u8; 2],
    pub byte_length: u64,
    pub sections: Vec<WorkspaceMemoryGenerationSectionV1>,
}

impl WorkspaceMemoryGenerationDirectoryV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_MEMORY_GENERATION_SEGMENT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("workspace memory generation directory schema mismatch".to_owned());
        }
        if self.workspace_identity.trim().is_empty() {
            return Err("workspace memory generation directory identity is empty".to_owned());
        }
        validate_qualified_blake3(
            &self.generation_digest,
            "workspace memory generation directory generation digest",
        )?;
        if self.active_epoch == 0 || self.byte_length == 0 {
            return Err(
                "workspace memory generation directory epoch and byte length must be positive"
                    .to_owned(),
            );
        }
        let required = [
            WorkspaceMemoryGenerationSectionKindV1::GenerationEvidence,
            WorkspaceMemoryGenerationSectionKindV1::ProjectResolutions,
            WorkspaceMemoryGenerationSectionKindV1::OwnerDirectory,
            WorkspaceMemoryGenerationSectionKindV1::OwnerBytes,
            WorkspaceMemoryGenerationSectionKindV1::LexicalIndex,
            WorkspaceMemoryGenerationSectionKindV1::SelectorIndex,
            WorkspaceMemoryGenerationSectionKindV1::GraphRelations,
        ];
        if self.sections.len() != required.len() {
            return Err(
                "workspace memory generation directory must contain every v1 section exactly once"
                    .to_owned(),
            );
        }
        let mut kinds = std::collections::BTreeSet::new();
        let mut previous_end = 0_u64;
        for section in &self.sections {
            if !kinds.insert(section.kind) {
                return Err(
                    "workspace memory generation directory contains a duplicate section".to_owned(),
                );
            }
            if section.offset < previous_end {
                return Err(
                    "workspace memory generation directory sections overlap or are unordered"
                        .to_owned(),
                );
            }
            let end = section.offset.checked_add(section.length).ok_or_else(|| {
                "workspace memory generation directory section range overflows".to_owned()
            })?;
            if end > self.byte_length {
                return Err(
                    "workspace memory generation directory section exceeds segment bounds"
                        .to_owned(),
                );
            }
            validate_qualified_blake3(
                &section.digest,
                "workspace memory generation directory section digest",
            )?;
            previous_end = end;
        }
        if required.iter().any(|kind| !kinds.contains(kind)) {
            return Err(
                "workspace memory generation directory omits a required v1 section".to_owned(),
            );
        }
        Ok(())
    }
}

fn validate_qualified_blake3(value: &str, subject: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("blake3-256:")
        .ok_or_else(|| format!("{subject} is unqualified"))?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{subject} is malformed"));
    }
    Ok(())
}

pub fn has_current_workspace_memory_generation_contract(bytes: &[u8]) -> bool {
    bytes.starts_with(WORKSPACE_MEMORY_GENERATION_SEGMENT_MAGIC)
}
