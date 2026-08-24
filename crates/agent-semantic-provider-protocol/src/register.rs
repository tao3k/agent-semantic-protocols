use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CompiledProviderRoute, ProviderRouteSpec};

pub const PROVIDER_REGISTER_SCHEMA_VERSION: &str = "1";
pub const PROVIDER_REGISTER_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-register.request";
pub const PROVIDER_REGISTER_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-register.response";
const BUILTIN_PROVIDER_REGISTER_JSON: &str =
    include_str!(concat!(env!("OUT_DIR"), "/provider-register.resolved.json"));

pub fn builtin_provider_register_json() -> &'static str {
    BUILTIN_PROVIDER_REGISTER_JSON
}

pub fn builtin_provider_registrations() -> Result<Vec<ProviderRegistrationDocument>, String> {
    let register: Value = serde_json::from_str(BUILTIN_PROVIDER_REGISTER_JSON)
        .map_err(|error| format!("embedded provider register is invalid JSON: {error}"))?;
    let providers = register
        .get("providers")
        .and_then(Value::as_array)
        .ok_or_else(|| "embedded provider register must declare providers".to_owned())?;
    providers
        .iter()
        .map(|registration| {
            let object = registration
                .as_object()
                .ok_or_else(|| "embedded provider registration must be an object".to_owned())?;
            let provider = ProviderRegistrationDocument {
                language_id: required_string(object, "languageId")?.to_owned(),
                provider_id: required_string(object, "providerId")?.to_owned(),
                registration: registration.clone(),
            };
            provider.validate()?;
            Ok(provider)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRegistrationDocument {
    pub language_id: String,
    pub provider_id: String,
    pub registration: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSourceInventory {
    pub package_roots: Vec<String>,
    pub config_files: Vec<String>,
    pub source_extensions: Vec<String>,
    pub project_resolution: Option<ProviderProjectInventory>,
    pub document_resolution: Option<ProviderDocumentInventory>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectInventory {
    pub entry_markers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDocumentInventory {
    pub extensions: Vec<String>,
    pub supports_git_candidates: bool,
}

impl ProviderRegistrationDocument {
    pub fn validate(&self) -> Result<(), String> {
        validate_identity("languageId", &self.language_id)?;
        validate_identity("providerId", &self.provider_id)?;
        let registration = self
            .registration
            .as_object()
            .ok_or_else(|| "provider registration must be a JSON object".to_owned())?;
        validate_matching_field(registration, "languageId", &self.language_id)?;
        validate_matching_field(registration, "providerId", &self.provider_id)
    }

    /// Compile the provider-owned semantic routes carried by an installed
    /// capability descriptor. Built-in seed identities deliberately do not
    /// carry routes; they constrain identity but are not executable catalog
    /// entries.
    pub fn compiled_routes(&self) -> Result<Vec<CompiledProviderRoute>, String> {
        self.validate()?;
        self.validate_capability_metadata()?;
        let routes = self
            .registration
            .get("routes")
            .and_then(Value::as_array)
            .ok_or_else(|| "installed provider capability must declare routes".to_owned())?;
        if routes.is_empty() {
            return Err("installed provider capability routes must not be empty".to_owned());
        }
        routes
            .iter()
            .map(|route| {
                let spec: ProviderRouteSpec = serde_json::from_value(route.clone())
                    .map_err(|error| format!("provider route is invalid: {error}"))?;
                if spec.target.language_id != self.language_id
                    || spec.target.provider_id != self.provider_id
                {
                    return Err(format!(
                        "provider route target drift: expected {}/{}, got {}/{}",
                        self.language_id,
                        self.provider_id,
                        spec.target.language_id,
                        spec.target.provider_id
                    ));
                }
                spec.compile().map_err(|error| error.to_string())
            })
            .collect()
    }

    pub fn namespace(&self) -> Result<&str, String> {
        self.registration
            .get("namespace")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "installed provider capability must declare namespace".to_owned())
    }

    pub fn source_inventory(&self) -> Result<ProviderSourceInventory, String> {
        let inventory = self
            .registration
            .get("sourceInventory")
            .ok_or_else(|| "installed provider capability must declare sourceInventory".to_owned())?
            .clone();
        let inventory: ProviderSourceInventory = serde_json::from_value(inventory)
            .map_err(|error| format!("provider sourceInventory is invalid: {error}"))?;
        if inventory.source_extensions.is_empty()
            || (inventory.project_resolution.is_none() && inventory.document_resolution.is_none())
        {
            return Err(
                "provider sourceInventory must declare extensions and at least one resolution capability"
                    .to_owned(),
            );
        }
        Ok(inventory)
    }

    pub fn registration_field(&self, field: &str) -> Result<&Value, String> {
        self.registration
            .get(field)
            .ok_or_else(|| format!("installed provider capability must declare {field}"))
    }

    fn validate_capability_metadata(&self) -> Result<(), String> {
        self.namespace()?;
        self.source_inventory()?;
        self.registration_field("searchCapabilities")?;
        self.registration_field("queryPackDescriptor")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProviderRegisterOperation {
    Initialize {
        providers: Vec<ProviderRegistrationDocument>,
    },
    Register {
        provider: ProviderRegistrationDocument,
    },
    Unregister {
        provider_id: String,
    },
    List,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRegisterRequest {
    pub schema_id: String,
    pub schema_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_generation: Option<u64>,
    pub request: ProviderRegisterOperation,
}

impl ProviderRegisterRequest {
    pub fn validate(&self) -> Result<(), String> {
        validate_schema(
            &self.schema_id,
            &self.schema_version,
            PROVIDER_REGISTER_REQUEST_SCHEMA_ID,
        )?;
        match &self.request {
            ProviderRegisterOperation::Initialize { providers } => {
                for provider in providers {
                    provider.validate()?;
                }
                reject_duplicate_providers(providers)
            }
            ProviderRegisterOperation::Register { provider } => {
                provider.compiled_routes().map(|_| ())
            }
            ProviderRegisterOperation::Unregister { provider_id } => {
                validate_identity("providerId", provider_id)
            }
            ProviderRegisterOperation::List => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRegisterSnapshot {
    pub generation: u64,
    pub digest: String,
    pub providers: Vec<ProviderRegistrationDocument>,
}

impl ProviderRegisterSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.digest.is_empty() {
            return Err("provider register digest must not be empty".to_owned());
        }
        for provider in &self.providers {
            provider.validate()?;
        }
        reject_duplicate_providers(&self.providers)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProviderRegisterResult {
    Snapshot {
        snapshot: ProviderRegisterSnapshot,
    },
    GenerationConflict {
        actual_generation: u64,
    },
    Rejected {
        reason_kind: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRegisterResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub result: ProviderRegisterResult,
}

impl ProviderRegisterResponse {
    pub fn validate(&self) -> Result<(), String> {
        validate_schema(
            &self.schema_id,
            &self.schema_version,
            PROVIDER_REGISTER_RESPONSE_SCHEMA_ID,
        )?;
        match &self.result {
            ProviderRegisterResult::Snapshot { snapshot } => snapshot.validate(),
            ProviderRegisterResult::GenerationConflict { .. } => Ok(()),
            ProviderRegisterResult::Rejected {
                reason_kind,
                message,
            } => {
                if reason_kind.is_empty() || message.is_empty() {
                    return Err(
                        "provider register rejection must be typed and non-empty".to_owned()
                    );
                }
                Ok(())
            }
        }
    }
}

fn validate_schema(schema_id: &str, schema_version: &str, expected: &str) -> Result<(), String> {
    if schema_id != expected {
        return Err(format!("expected schemaId `{expected}`, got `{schema_id}`"));
    }
    if schema_version != PROVIDER_REGISTER_SCHEMA_VERSION {
        return Err(format!(
            "expected schemaVersion `{PROVIDER_REGISTER_SCHEMA_VERSION}`, got `{schema_version}`"
        ));
    }
    Ok(())
}

fn validate_identity(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!(
            "{field} must use non-empty lowercase kebab-case identity"
        ));
    }
    Ok(())
}

fn validate_matching_field(
    registration: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), String> {
    let actual = registration
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("provider registration must declare string `{field}`"))?;
    if actual != expected {
        return Err(format!(
            "provider registration {field} `{actual}` does not match `{expected}`"
        ));
    }
    Ok(())
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("provider registration must declare string `{field}`"))
}

fn reject_duplicate_providers(providers: &[ProviderRegistrationDocument]) -> Result<(), String> {
    let mut provider_ids = std::collections::BTreeSet::new();
    for provider in providers {
        if !provider_ids.insert(provider.provider_id.as_str()) {
            return Err(format!("duplicate providerId `{}`", provider.provider_id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider(language_id: &str, provider_id: &str) -> ProviderRegistrationDocument {
        ProviderRegistrationDocument {
            language_id: language_id.to_owned(),
            provider_id: provider_id.to_owned(),
            registration: json!({
                "languageId": language_id,
                "providerId": provider_id,
                "namespace": language_id,
                "sourceInventory": {
                    "packageRoots": [],
                    "configFiles": [],
                    "sourceExtensions": [format!(".{language_id}")],
                    "projectResolution": {
                        "entryMarkers": [format!("{language_id}.project")]
                    },
                    "documentResolution": null
                },
                "searchCapabilities": {
                    "ownerItems": true,
                    "semanticFacts": true,
                    "dependencyTopology": false,
                    "dependencyTopologyMetadata": false
                },
                "queryPackDescriptor": {},
                "routes": [{
                    "schemaId": "agent.semantic-protocols.provider-route",
                    "schemaVersion": "1",
                    "routeId": format!("{language_id}.search.owner"),
                    "operation": "search.owner",
                    "authority": "asp-server",
                    "target": {
                        "languageId": language_id,
                        "providerId": provider_id
                    },
                    "inputs": [],
                    "requirements": [],
                    "effects": {
                        "access": "read",
                        "idempotent": true,
                        "cancellable": true,
                        "concurrency": "shared-read",
                        "streaming": false
                    },
                    "output": {
                        "schemaId": "agent.semantic-protocols.search-packet",
                        "mediaType": "application/json"
                    },
                    "failureSchemaIds": ["agent.semantic-protocols.route-failure"],
                    "cache": {
                        "authority": "asp-server",
                        "scope": "workspace",
                        "keySlots": []
                    },
                    "telemetry": {
                        "spanName": "asp.route.search.owner",
                        "attributeSlots": []
                    }
                }]
            }),
        }
    }

    #[test]
    fn external_provider_uses_the_same_registration_document_contract() {
        let request = ProviderRegisterRequest {
            schema_id: PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
            expected_generation: Some(7),
            request: ProviderRegisterOperation::Register {
                provider: provider("external-test", "asp-external-test"),
            },
        };
        request.validate().expect("external provider registration");
        let encoded = serde_json::to_value(&request).expect("serialize request");
        let decoded: ProviderRegisterRequest =
            serde_json::from_value(encoded).expect("deserialize request");
        assert_eq!(decoded, request);
    }

    #[test]
    fn initialize_rejects_duplicate_provider_identity() {
        let request = ProviderRegisterRequest {
            schema_id: PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
            expected_generation: None,
            request: ProviderRegisterOperation::Initialize {
                providers: vec![provider("rust", "asp-rust"), provider("rust-2", "asp-rust")],
            },
        };
        assert_eq!(
            request.validate(),
            Err("duplicate providerId `asp-rust`".to_owned())
        );
    }

    #[test]
    fn registration_identity_must_match_the_schema_document() {
        let mut registration = provider("python", "asp-python");
        registration.registration["providerId"] = json!("asp-rust");
        assert_eq!(
            registration.validate(),
            Err(
                "provider registration providerId `asp-rust` does not match `asp-python`"
                    .to_owned()
            )
        );
    }

    #[test]
    fn installed_capability_requires_source_inventory_owned_by_the_client_server() {
        let mut registration = provider("rust", "asp-rust");
        registration
            .registration
            .as_object_mut()
            .expect("registration object")
            .remove("sourceInventory");
        assert_eq!(
            registration.compiled_routes(),
            Err("installed provider capability must declare sourceInventory".to_owned())
        );
    }

    #[test]
    fn builtin_providers_are_loaded_from_the_schema_register() {
        let providers = builtin_provider_registrations().expect("builtin provider register");
        assert!(
            providers
                .iter()
                .any(|provider| provider.provider_id == "asp-rust")
        );
        assert!(
            providers
                .iter()
                .any(|provider| provider.provider_id == "asp-python")
        );
        assert_eq!(providers.len(), 7);
    }
}
