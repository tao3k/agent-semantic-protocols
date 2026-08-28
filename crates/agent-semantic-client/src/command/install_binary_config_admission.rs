use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_artifacts::hook_generation::{
    HookGenerationCandidate, HookGenerationPublicationReceipt, PreparedHookGeneration,
    commit_hook_generation, prepare_hook_generation,
};

static CANDIDATE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn admit_embedded_hook_config() -> Result<(), String> {
    agent_semantic_config::default_hook_client_config_file()
        .map(|_| ())
        .map_err(|error| {
            format!(
                "ASP binary/config publication admission failed before artifact switch: {error}"
            )
        })
}

/// Publish the Hook matcher contract embedded in the installing executable.
///
/// Binary and matcher config are one compatibility generation.  Keeping this
/// publication in the canonical binary installer means a stale matcher can
/// never block the command that repairs the pair; Runtime Server availability
/// is deliberately not part of this local recovery edge.
pub(super) fn publish_embedded_hook_config(protocol_home: &Path) -> Result<&'static str, String> {
    let path = protocol_home.join("hooks/config.toml");
    let status = super::managed_hook_config::materialize(&path).map_err(|error| {
        format!(
            "ASP binary/config publication failed for {} after binary switch: {error}",
            path.display()
        )
    })?;
    Ok(status.as_str())
}

pub(super) struct HookGenerationInstallReceipt {
    pub publication: HookGenerationPublicationReceipt,
    pub config_source_status: &'static str,
    pub evaluator_validation_elapsed_micros: u128,
}

struct PreparedHookGenerationInstall {
    prepared: PreparedHookGeneration,
    config_source_status: &'static str,
}

/// Compile, validate, and atomically commit a complete HookGeneration.
///
/// The candidate is built from immutable snapshots. The mutable installed
/// config source is materialized for operators, but it is never read by the
/// serving evaluator and therefore cannot change the active generation.
pub(super) async fn publish_embedded_hook_generation(
    protocol_home: &Path,
    evaluator_binary: &Path,
) -> Result<HookGenerationInstallReceipt, String> {
    let protocol_home = protocol_home.to_path_buf();
    let candidate_home = protocol_home.clone();
    let evaluator_binary = evaluator_binary.to_path_buf();
    let prepared = tokio::task::spawn_blocking(move || {
        prepare_embedded_hook_generation_blocking(&candidate_home, &evaluator_binary)
    })
    .await
    .map_err(|error| format!("HookGeneration candidate task failed: {error}"))??;
    let validation_started = tokio::time::Instant::now();
    agent_semantic_hook::candidate_validation::validate_hook_evaluator_candidate(
        agent_semantic_hook::candidate_validation::HookEvaluatorCandidateValidation {
            evaluator_path: &prepared.prepared.receipt.evaluator_path,
            generation_path: &prepared.prepared.receipt.generation_path,
            generation_digest: &prepared.prepared.receipt.generation_digest,
            state_home: &protocol_home,
        },
    )
    .await?;
    let evaluator_validation_elapsed_micros = validation_started.elapsed().as_micros();
    let publication = tokio::task::spawn_blocking(move || {
        commit_hook_generation(&protocol_home, &prepared.prepared)
            .map(|publication| (publication, prepared.config_source_status))
    })
    .await
    .map_err(|error| format!("HookGeneration commit task failed: {error}"))??;
    Ok(HookGenerationInstallReceipt {
        publication: publication.0,
        config_source_status: publication.1,
        evaluator_validation_elapsed_micros,
    })
}

fn prepare_embedded_hook_generation_blocking(
    protocol_home: &Path,
    evaluator_binary: &Path,
) -> Result<PreparedHookGenerationInstall, String> {
    admit_embedded_hook_config()?;
    let config_source_status = publish_embedded_hook_config(protocol_home)?;
    let config = agent_semantic_config::default_hook_client_config_template().into_bytes();
    let registry = agent_semantic_config::embedded_agent_assets::embedded_agent_assets()
        .iter()
        .find(|asset| asset.file_name == "config.toml")
        .ok_or_else(|| "embedded HookGeneration registry is missing config.toml".to_owned())?
        .contents;
    let registry_source = std::str::from_utf8(registry)
        .map_err(|error| format!("embedded HookGeneration registry is not UTF-8: {error}"))?;
    agent_semantic_config::agent_route_registry::parse_agent_route_registry(
        registry_source,
        "embedded HookGeneration registry",
    )
    .map_err(|error| format!("validate HookGeneration registry candidate: {error}"))?;
    let candidate_root = protocol_home.join("hooks/candidates").join(format!(
        "candidate-{}-{}",
        std::process::id(),
        CANDIDATE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let candidate_config = candidate_root.join("config.toml");
    write_candidate(&candidate_config, &config)?;
    let result = (|| {
        let compiled = agent_semantic_hook::compile_hook_matcher_generation(
            &candidate_config,
            &candidate_root,
        )
        .map_err(|error| format!("compile HookGeneration candidate: {error}"))?;
        agent_semantic_hook::validate_compiled_hook_matcher(&compiled.bytes)
            .map_err(|error| format!("validate HookGeneration matcher candidate: {error}"))?;
        let prepared = prepare_hook_generation(
            protocol_home,
            HookGenerationCandidate {
                evaluator_binary,
                config: &config,
                compiled_matcher: &compiled.bytes,
                registry,
            },
        )?;
        Ok(PreparedHookGenerationInstall {
            prepared,
            config_source_status,
        })
    })();
    // Scratch cleanup is outside the committed generation and cannot change
    // either the prior current pointer on failure or the new pointer on success.
    let _ = std::fs::remove_dir_all(&candidate_root);
    result
}

fn write_candidate(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("HookGeneration candidate has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create HookGeneration candidate directory {}: {error}",
            parent.display()
        )
    })?;
    std::fs::write(path, bytes)
        .map_err(|error| format!("write HookGeneration candidate {}: {error}", path.display()))
}

#[cfg(test)]
#[path = "../../tests/unit/install_binary_config_admission.rs"]
mod tests;
