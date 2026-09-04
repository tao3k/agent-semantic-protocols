//! Runtime provider catalog identity and atomic publication.

use std::io::ErrorKind;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProviderCatalogIdentity {
    pub schema_id: String,
    pub schema_version: String,
    pub catalog_generation: String,
    pub binary_artifact_digest: String,
    pub install_registry_digest: String,
}

impl RuntimeProviderCatalogIdentity {
    pub fn validate(&self) -> Result<(), String> {
        validate_provider_catalog_digest("binaryArtifactDigest", &self.binary_artifact_digest)?;
        validate_provider_catalog_digest("installRegistryDigest", &self.install_registry_digest)?;
        let derived = runtime_provider_catalog_generation(
            &self.binary_artifact_digest,
            &self.install_registry_digest,
        );
        if self.schema_id != "agent.semantic-protocols.runtime-provider-catalog"
            || self.schema_version != "1"
            || self.catalog_generation != derived
        {
            return Err("runtime provider catalog identity drift".to_owned());
        }
        Ok(())
    }
}

fn validate_provider_catalog_digest(field: &str, digest: &str) -> Result<(), String> {
    let Some((algorithm, value)) = digest.split_once(':') else {
        return Err(format!(
            "runtime provider catalog {field} is not an integrity reference"
        ));
    };
    if !matches!(algorithm, "blake3-256" | "sha256")
        || value.len() != 64
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!("runtime provider catalog {field} is invalid"));
    }
    Ok(())
}

pub fn load_runtime_provider_catalog_identity(
    state_home: &Path,
) -> Result<Option<RuntimeProviderCatalogIdentity>, String> {
    let path = state_home.join("runtime/provider-catalog.v1.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read runtime provider catalog identity {}: {error}",
                path.display()
            ));
        }
    };
    let identity: RuntimeProviderCatalogIdentity =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "parse runtime provider catalog identity {}: {error}",
                path.display()
            )
        })?;
    identity.validate()?;
    Ok(Some(identity))
}

pub fn runtime_provider_catalog_generation(
    binary_artifact_digest: &str,
    install_registry_digest: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.runtime-provider-catalog.v1\0");
    hasher.update(binary_artifact_digest.as_bytes());
    hasher.update(b"\0");
    hasher.update(install_registry_digest.as_bytes());
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeProviderCatalogPublicationObservation {
    Absent,
    Current(RuntimeProviderCatalogIdentity),
    Obsolete { content_digest: String },
}

fn observe_runtime_provider_catalog_for_publication(
    state_home: &Path,
) -> Result<RuntimeProviderCatalogPublicationObservation, String> {
    let path = state_home.join("runtime/provider-catalog.v1.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(RuntimeProviderCatalogPublicationObservation::Absent);
        }
        Err(error) => {
            return Err(format!(
                "read runtime provider catalog publication state {}: {error}",
                path.display()
            ));
        }
    };
    if let Ok(identity) = serde_json::from_slice::<RuntimeProviderCatalogIdentity>(&bytes)
        && identity.validate().is_ok()
    {
        return Ok(RuntimeProviderCatalogPublicationObservation::Current(
            identity,
        ));
    }
    Ok(RuntimeProviderCatalogPublicationObservation::Obsolete {
        content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
    })
}

pub fn publish_runtime_provider_catalog(
    state_home: &Path,
    binary_artifact_digest: &str,
    install_registry_digest: &str,
) -> Result<String, String> {
    for _ in 0..3 {
        let observed = observe_runtime_provider_catalog_for_publication(state_home)?;
        let desired =
            runtime_provider_catalog_generation(binary_artifact_digest, install_registry_digest);
        if matches!(
            &observed,
            RuntimeProviderCatalogPublicationObservation::Current(identity)
                if identity.catalog_generation == desired
        ) {
            return Ok(desired);
        }
        match publish_runtime_provider_catalog_from_observation(
            state_home,
            binary_artifact_digest,
            install_registry_digest,
            &observed,
        ) {
            Ok(generation) => return Ok(generation),
            Err(error) if error.contains("publication conflict") => continue,
            Err(error) => return Err(error),
        }
    }
    Err("runtime provider catalog publication did not converge after 3 CAS attempts".to_owned())
}

fn publish_runtime_provider_catalog_from_observation(
    state_home: &Path,
    binary_artifact_digest: &str,
    install_registry_digest: &str,
    expected: &RuntimeProviderCatalogPublicationObservation,
) -> Result<String, String> {
    let artifact_root = state_home.join("runtime/artifacts");
    let _guard = crate::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
        &artifact_root,
    )?;
    let observed = observe_runtime_provider_catalog_for_publication(state_home)?;
    if &observed != expected {
        return Err(
            "runtime provider catalog publication conflict: observed bytes changed".to_owned(),
        );
    }
    write_runtime_provider_catalog_identity(
        state_home,
        binary_artifact_digest,
        install_registry_digest,
    )
}

pub fn publish_runtime_provider_catalog_cas(
    state_home: &Path,
    binary_artifact_digest: &str,
    install_registry_digest: &str,
    expected_generation: Option<&str>,
) -> Result<String, String> {
    let artifact_root = state_home.join("runtime/artifacts");
    let _guard = crate::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
        &artifact_root,
    )?;
    let observed = load_runtime_provider_catalog_identity(state_home)?;
    let observed_generation = observed
        .as_ref()
        .map(|identity| identity.catalog_generation.as_str());
    if observed_generation != expected_generation {
        return Err(format!(
            "runtime provider catalog publication conflict: expectedGeneration={} observedGeneration={}",
            expected_generation.unwrap_or("absent"),
            observed_generation.unwrap_or("absent")
        ));
    }
    write_runtime_provider_catalog_identity(
        state_home,
        binary_artifact_digest,
        install_registry_digest,
    )
}

fn write_runtime_provider_catalog_identity(
    state_home: &Path,
    binary_artifact_digest: &str,
    install_registry_digest: &str,
) -> Result<String, String> {
    let catalog_generation =
        runtime_provider_catalog_generation(binary_artifact_digest, install_registry_digest);
    let identity = RuntimeProviderCatalogIdentity {
        schema_id: "agent.semantic-protocols.runtime-provider-catalog".to_owned(),
        schema_version: "1".to_owned(),
        catalog_generation: catalog_generation.clone(),
        binary_artifact_digest: binary_artifact_digest.to_owned(),
        install_registry_digest: install_registry_digest.to_owned(),
    };
    let path = state_home.join("runtime/provider-catalog.v1.json");
    let parent = path.parent().ok_or_else(|| {
        format!(
            "runtime provider catalog path has no parent: {}",
            path.display()
        )
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create runtime provider catalog root {}: {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec_pretty(&identity)
        .map_err(|error| format!("encode runtime provider catalog: {error}"))?;
    let temporary = parent.join(format!(".provider-catalog.{}.tmp", catalog_generation));
    std::fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "stage runtime provider catalog {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &path).map_err(|error| {
        format!(
            "publish runtime provider catalog {}: {error}",
            path.display()
        )
    })?;
    Ok(catalog_generation)
}
