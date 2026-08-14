#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderNativeExactResolution {
    pub(crate) schema_id: String,
    pub(crate) schema_version: String,
    pub(crate) language_id: String,
    pub(crate) provider_id: String,
    pub(crate) owner_path: String,
    pub(crate) requested_structural_selector: String,
    pub(crate) resolution_state: String,
    pub(crate) reason_kind: String,
    pub(crate) active_generation_digest: String,
    pub(crate) root_digest: String,
    pub(crate) item_kind: String,
    pub(crate) item_name: String,
    #[serde(default)]
    pub(crate) candidates: Vec<String>,
    #[serde(default)]
    pub(crate) actual_kinds: Vec<String>,
}

pub(crate) struct ProviderExactResolutionFacts {
    pub(crate) language_id: String,
    pub(crate) provider_id: String,
    pub(crate) owner_path: String,
    pub(crate) structural_selector: String,
    pub(crate) resolution_state: String,
    pub(crate) reason_kind: String,
    pub(crate) active_generation_digest: String,
    pub(crate) root_digest: String,
    pub(crate) item_kind: String,
    pub(crate) item_name: String,
    pub(crate) candidates: Vec<String>,
    pub(crate) actual_kinds: Vec<String>,
}

pub(crate) fn resolution_from_facts(
    facts: ProviderExactResolutionFacts,
) -> ProviderNativeExactResolution {
    ProviderNativeExactResolution {
        schema_id: "agent.semantic-protocols.provider-native-exact-projection".to_owned(),
        schema_version: "1".to_owned(),
        language_id: facts.language_id.clone(),
        provider_id: facts.provider_id,
        owner_path: facts.owner_path.clone(),
        requested_structural_selector: facts.structural_selector,
        resolution_state: facts.resolution_state.clone(),
        reason_kind: facts.reason_kind,
        active_generation_digest: facts.active_generation_digest,
        root_digest: facts.root_digest,
        item_kind: facts.item_kind.clone(),
        item_name: facts.item_name,
        candidates: facts.candidates,
        actual_kinds: facts.actual_kinds,
    }
}

pub(crate) fn validate_resolution(
    resolution: &ProviderNativeExactResolution,
    language_id: &str,
    provider_id: &str,
    owner_path: &str,
    structural_selector: &str,
) -> Result<(), String> {
    validate_field(
        "schemaId",
        &resolution.schema_id,
        "agent.semantic-protocols.provider-native-exact-projection",
    )?;
    validate_field("schemaVersion", &resolution.schema_version, "1")?;
    validate_field("languageId", &resolution.language_id, language_id)?;
    validate_field("providerId", &resolution.provider_id, provider_id)?;
    validate_field("ownerPath", &resolution.owner_path, owner_path)?;
    validate_field(
        "requestedStructuralSelector",
        &resolution.requested_structural_selector,
        structural_selector,
    )?;
    if resolution.resolution_state.is_empty() || resolution.reason_kind.is_empty() {
        return Err("exact-selector semantic resolution is incomplete".to_owned());
    }
    validate_generation_digest(&resolution.active_generation_digest)?;
    validate_root_digest(&resolution.root_digest)?;
    Ok(())
}

pub(crate) fn render_provider_exact_resolution(
    resolution: &ProviderNativeExactResolution,
) -> String {
    render_human_diagnostic(resolution)
}

fn render_human_diagnostic(resolution: &ProviderNativeExactResolution) -> String {
    let mut diagnostic = format!(
        "exact source query state={} reasonKind={} ownerPath={} itemKind={} itemName={}",
        resolution.resolution_state,
        resolution.reason_kind,
        resolution.owner_path,
        resolution.item_kind,
        resolution.item_name,
    );
    diagnostic.push_str(" rootDigest=");
    diagnostic.push_str(&resolution.root_digest);
    diagnostic.push_str(" activeGenerationDigest=");
    diagnostic.push_str(&resolution.active_generation_digest);
    if !resolution.candidates.is_empty() {
        diagnostic.push_str(" candidates=");
        diagnostic.push_str(&resolution.candidates.join(","));
    }
    if !resolution.actual_kinds.is_empty() {
        diagnostic.push_str(" actualKinds=");
        diagnostic.push_str(&resolution.actual_kinds.join(","));
    }
    diagnostic
}

fn validate_field(field: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "exact-selector {field} mismatch: expected={expected} actual={actual}"
        ));
    }
    Ok(())
}

fn validate_generation_digest(digest: &str) -> Result<(), String> {
    let Some(value) = digest.strip_prefix("blake3-256:") else {
        return Err("exact-selector activeGenerationDigest is not BLAKE3 authority".to_owned());
    };
    validate_hex_digest("activeGenerationDigest", value)
}

fn validate_root_digest(digest: &str) -> Result<(), String> {
    validate_hex_digest("rootDigest", digest)
}

fn validate_hex_digest(field: &str, digest: &str) -> Result<(), String> {
    if digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!(
            "exact-selector {field} is not a 256-bit hex digest"
        ))
    }
}
