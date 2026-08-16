use crate::runtime_resident_read::RuntimeResidentReadClient;
use crate::runtime_resident_read::RuntimeResidentReadWorkCounters;
use crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead;

const SCHEMA_ID: &str =
    "agent-semantic-protocols.runtime-merkle-owner-proof-qualification-receipt.v1";
const SCHEMA_VERSION: &str = "1";
const PROOF_DIGEST_DOMAIN: &[u8] = b"asp.runtime-merkle-owner-inclusion-proof.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMerkleOwnerProofEvidenceLayer {
    Scenario,
    LiveCorpus,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeMerkleOwnerProofQualificationReceiptV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub evidence_layer: RuntimeMerkleOwnerProofEvidenceLayer,
    pub runtime_ecosystem: String,
    pub read_mode: String,
    pub case_id: String,
    pub resource_id: Option<String>,
    pub language_id: String,
    pub provider_id: String,
    pub workspace_identity: String,
    pub generation_digest: Option<String>,
    pub root_digest: Option<String>,
    pub owner_path: Option<String>,
    pub owner_content_digest: Option<String>,
    pub owner_subtree_digest: Option<String>,
    pub structural_selector: Option<String>,
    pub proof_digest: Option<String>,
    pub proof_step_count: Option<usize>,
    pub elapsed_micros: u64,
    pub work_counters: RuntimeResidentReadWorkCounters,
    pub status: String,
    pub reason_kind: Option<String>,
}

impl RuntimeMerkleOwnerProofQualificationReceiptV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn qualified(
        evidence_layer: RuntimeMerkleOwnerProofEvidenceLayer,
        case_id: impl Into<String>,
        resource_id: Option<String>,
        language_id: impl Into<String>,
        provider_id: impl Into<String>,
        structural_selector: impl Into<String>,
        elapsed_micros: u64,
        work_counters: RuntimeResidentReadWorkCounters,
        read: WorkspaceRuntimeMerkleOwnerRead,
    ) -> Result<Self, String> {
        let case_id = require_nonempty("caseId", case_id.into())?;
        let language_id = require_nonempty("languageId", language_id.into())?;
        let provider_id = require_nonempty("providerId", provider_id.into())?;
        let structural_selector =
            require_nonempty("structuralSelector", structural_selector.into())?;
        if elapsed_micros >= 1_000 {
            return Err(format!(
                "Runtime Merkle owner proof exceeded the synchronous mmap budget: elapsedMicros={elapsed_micros} budgetExclusiveMicros=1000"
            ));
        }
        require_zero_work(work_counters)?;

        let WorkspaceRuntimeMerkleOwnerRead::Owner {
            workspace_identity,
            generation_digest,
            root_digest,
            owner_path,
            source_blob_digest,
            owner_subtree_digest,
            inclusion_proof,
            ..
        } = read
        else {
            return Err(
                "Runtime Merkle owner proof is missing: reasonKind=runtime-merkle-owner-proof-missing"
                    .to_owned(),
            );
        };
        let proof_payload = serde_json::to_vec(&inclusion_proof)
            .map_err(|error| format!("encode Runtime Merkle inclusion proof: {error}"))?;
        let proof_digest =
            agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest_v1(
                PROOF_DIGEST_DOMAIN,
                &[&proof_payload],
            );
        let proof_digest = format!("blake3-256:{}", proof_digest.as_str());

        Ok(Self {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            evidence_layer,
            runtime_ecosystem: "tokio".to_owned(),
            read_mode: "synchronous-mmap".to_owned(),
            case_id,
            resource_id,
            language_id,
            provider_id,
            workspace_identity,
            generation_digest: Some(generation_digest),
            root_digest: Some(root_digest),
            owner_path: Some(owner_path),
            owner_content_digest: Some(source_blob_digest),
            owner_subtree_digest: Some(owner_subtree_digest),
            structural_selector: Some(structural_selector),
            proof_digest: Some(proof_digest),
            proof_step_count: Some(inclusion_proof.len()),
            elapsed_micros,
            work_counters,
            status: "qualified".to_owned(),
            reason_kind: None,
        })
    }
}

impl RuntimeResidentReadClient {
    #[allow(clippy::too_many_arguments)]
    pub fn qualify_merkle_owner_proof(
        &self,
        evidence_layer: RuntimeMerkleOwnerProofEvidenceLayer,
        case_id: impl Into<String>,
        resource_id: Option<String>,
        language_id: impl Into<String>,
        provider_id: impl Into<String>,
        owner_path: &str,
        structural_selector: impl Into<String>,
    ) -> Result<RuntimeMerkleOwnerProofQualificationReceiptV1, String> {
        let case_id = case_id.into();
        let language_id = language_id.into();
        let started = std::time::Instant::now();
        let read = self.read_merkle_owner(owner_path)?;
        let elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let receipt = RuntimeMerkleOwnerProofQualificationReceiptV1::qualified(
            evidence_layer,
            case_id.clone(),
            resource_id,
            language_id.clone(),
            provider_id,
            structural_selector,
            elapsed_micros,
            self.work_counters(),
            read,
        )?;
        let telemetry_language_id =
            agent_semantic_client_core::LanguageId::from(language_id.as_str());
        self.try_record_read_observation(
            "runtime-merkle-owner-proof",
            "qualified",
            &case_id,
            Some(&telemetry_language_id),
            "owner-inclusion-proof",
            elapsed_micros,
            1_000,
            "within-budget",
        );
        Ok(receipt)
    }
}

fn require_nonempty(field: &str, value: String) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err(format!(
            "Runtime Merkle owner proof {field} must not be empty"
        ));
    }
    Ok(value)
}

fn require_zero_work(work: RuntimeResidentReadWorkCounters) -> Result<(), String> {
    if work.provider_process_count == 0
        && work.scheduler_task_count == 0
        && work.filesystem_read_count == 0
        && work.database_read_count == 0
        && work.socket_operation_count == 0
    {
        return Ok(());
    }
    Err(format!(
        "Runtime Merkle owner proof violated synchronous mmap authority: providerProcesses={} schedulerTasks={} filesystemReads={} databaseReads={} socketOperations={}",
        work.provider_process_count,
        work.scheduler_task_count,
        work.filesystem_read_count,
        work.database_read_count,
        work.socket_operation_count,
    ))
}
