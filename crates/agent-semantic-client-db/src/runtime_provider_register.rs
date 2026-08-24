use agent_semantic_client_protocol::{
    CLIENT_CATALOG_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientCapabilities,
    ClientMethod, ClientParameter, ClientParameterCardinality, ClientParameterSource,
    ClientParameterType, ClientProtocolCatalog, ClientTransport,
    SCHEMA_VERSION as CLIENT_SCHEMA_VERSION,
};
use agent_semantic_provider_protocol::{
    CompiledProviderRoute, PROVIDER_REGISTER_RESPONSE_SCHEMA_ID, PROVIDER_REGISTER_SCHEMA_VERSION,
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResponse,
    ProviderRegisterResult, ProviderRegisterSnapshot, ProviderRegistrationDocument,
};
use arc_swap::ArcSwap;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const PROVIDER_REGISTER_STATE_SCHEMA_ID: &str = "agent.semantic-protocols.provider-register.state";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedProviderRegisterState {
    schema_id: String,
    schema_version: String,
    providers: Vec<ProviderRegistrationDocument>,
}

pub struct RuntimeProviderRegister {
    state: ArcSwap<RuntimeProviderRegisterState>,
    writer: tokio::sync::Mutex<()>,
    identity_constraints: BTreeMap<String, ProviderRegistrationDocument>,
    store_path: Option<PathBuf>,
}

struct RuntimeProviderRegisterState {
    snapshot: Arc<ProviderRegisterSnapshot>,
    routes: BTreeMap<String, Arc<[CompiledProviderRoute]>>,
}

impl Default for RuntimeProviderRegister {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProviderRegister {
    pub fn new() -> Self {
        Self {
            state: ArcSwap::from_pointee(build_state(0, Vec::new()).expect("empty register")),
            writer: tokio::sync::Mutex::new(()),
            identity_constraints: BTreeMap::new(),
            store_path: None,
        }
    }

    pub fn from_seed(providers: Vec<ProviderRegistrationDocument>) -> Result<Self, String> {
        validate_provider_set(&providers)?;
        let identity_constraints = providers
            .iter()
            .cloned()
            .map(|provider| (provider.provider_id.clone(), provider))
            .collect();
        Ok(Self {
            state: ArcSwap::from_pointee(build_state(1, providers)?),
            writer: tokio::sync::Mutex::new(()),
            identity_constraints,
            store_path: None,
        })
    }

    pub async fn from_seed_with_store(
        identity_constraints: Vec<ProviderRegistrationDocument>,
        store_path: PathBuf,
    ) -> Result<Self, String> {
        let mut register = Self::from_seed(identity_constraints)?;
        let installed_capabilities = read_external_providers(&store_path).await?;
        for provider in &installed_capabilities {
            if let Some(identity) = register.identity_constraints.get(&provider.provider_id)
                && identity.language_id != provider.language_id
            {
                return Err(format!(
                    "persisted provider `{}` violates builtin language identity `{}`",
                    provider.provider_id, identity.language_id
                ));
            }
        }
        let mut providers = register
            .snapshot()
            .providers
            .iter()
            .cloned()
            .map(|provider| (provider.provider_id.clone(), provider))
            .collect::<BTreeMap<_, _>>();
        providers.extend(
            installed_capabilities
                .into_iter()
                .map(|provider| (provider.provider_id.clone(), provider)),
        );
        let mut providers = providers.into_values().collect::<Vec<_>>();
        providers.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        validate_provider_set(&providers)?;
        register.state = ArcSwap::from_pointee(build_state(1, providers)?);
        register.store_path = Some(store_path);
        Ok(register)
    }

    pub fn snapshot(&self) -> Arc<ProviderRegisterSnapshot> {
        Arc::clone(&self.state.load().snapshot)
    }

    /// Project the public, language-neutral client catalog from the same
    /// immutable generation that owns installed provider capabilities. This is a
    /// read-only projection: clients cannot register, launch, or retire a
    /// provider through this contract.
    pub fn client_protocol_catalog(
        &self,
        workspace_generation: impl Into<String>,
        transports: Vec<ClientTransport>,
    ) -> Result<ClientProtocolCatalog, String> {
        let state = self.state.load();
        let mut methods = state
            .snapshot
            .providers
            .iter()
            .flat_map(|provider| {
                state
                    .routes
                    .get(&provider.provider_id)
                    .into_iter()
                    .flat_map(|routes| routes.iter())
                    .filter_map(move |route| {
                        let spec = route.spec();
                        let request_schema_id =
                            spec.request_schema_id.as_deref().or_else(|| {
                                runtime_contract_schemas(provider, &spec.operation)
                                    .map(|(request_schema_id, _)| request_schema_id)
                            })?;
                        Some(ClientMethod {
                            method: spec.route_id.clone(),
                            route_id: spec.route_id.clone(),
                            request_schema_id: request_schema_id.to_owned(),
                            response_schema_id: spec.output.schema_id.clone(),
                            error_schema_ids: spec.failure_schema_ids.clone(),
                            parameters: spec
                                .inputs
                                .iter()
                                .map(|input| ClientParameter {
                                    name: input.name.clone(),
                                    value_type: client_parameter_type(input.value_type),
                                    cardinality: client_parameter_cardinality(input.cardinality),
                                    source: client_parameter_source(input.source),
                                })
                                .collect(),
                            cancellable: spec.effects.cancellable,
                            streaming: spec.effects.streaming,
                        })
                    })
            })
            .collect::<Vec<_>>();
        methods.sort_by(|left, right| left.method.cmp(&right.method));
        let catalog = ClientProtocolCatalog {
            schema_id: CLIENT_CATALOG_SCHEMA_ID.to_owned(),
            schema_version: CLIENT_SCHEMA_VERSION.to_owned(),
            protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
            protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
            catalog_generation: state.snapshot.digest.clone(),
            workspace_generation: workspace_generation.into(),
            transports,
            capabilities: ClientCapabilities {
                request_cancellation: methods.iter().any(|method| method.cancellable),
                events: true,
                streaming: methods.iter().any(|method| method.streaming),
                trace_context: true,
            },
            methods,
        };
        catalog
            .validate()
            .map_err(|error| format!("{}: {}", error.reason_kind, error.message))?;
        Ok(catalog)
    }

    pub fn compiled_routes(&self, provider_id: &str) -> Option<Arc<[CompiledProviderRoute]>> {
        self.state.load().routes.get(provider_id).map(Arc::clone)
    }

    /// Resolve one public ASP Client method from the same immutable Server-route
    /// generation used to publish the client catalog. Provider runtime
    /// operations are a separate southbound contract and do not gate these
    /// northbound methods.
    pub fn resolve_client_method(
        &self,
        method: &str,
    ) -> Result<(String, String, CompiledProviderRoute), String> {
        let state = self.state.load();
        let mut matches = state
            .snapshot
            .providers
            .iter()
            .filter_map(|provider| {
                state
                    .routes
                    .get(&provider.provider_id)
                    .map(|routes| (provider, routes))
            })
            .flat_map(|(provider, routes)| {
                routes.iter().filter_map(move |route| {
                    let spec = route.spec();
                    (spec.route_id == method).then(|| {
                        (
                            provider.language_id.clone(),
                            spec.operation.clone(),
                            route.clone(),
                        )
                    })
                })
            });
        let resolved = matches.next().ok_or_else(|| {
            format!(
                "state=route-missing reasonKind=method-not-in-installed-server-routes method={method}"
            )
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "state=route-ambiguous reasonKind=multiple-installed-client-methods method={method}"
            ));
        }
        Ok(resolved)
    }

    /// Return the unique installed capability descriptor for one language.
    /// Identity-only seeds are excluded because they are not executable.
    pub fn installed_capability(
        &self,
        language_id: &str,
    ) -> Result<ProviderRegistrationDocument, String> {
        let state = self.state.load();
        let mut providers = state.snapshot.providers.iter().filter(|provider| {
            provider.language_id == language_id && state.routes.contains_key(&provider.provider_id)
        });
        let provider = providers.next().ok_or_else(|| {
            format!(
                "state=provider-missing reasonKind=language-capability-not-installed languageId={language_id}"
            )
        })?;
        if providers.next().is_some() {
            return Err(format!(
                "state=provider-ambiguous reasonKind=multiple-installed-language-capabilities languageId={language_id}"
            ));
        }
        Ok(provider.clone())
    }

    /// Project every executable provider from one atomic register generation.
    /// Identity-only seeds are excluded because they are not Runtime clients.
    pub fn installed_capabilities(&self) -> Vec<ProviderRegistrationDocument> {
        let state = self.state.load();
        state
            .snapshot
            .providers
            .iter()
            .filter(|provider| state.routes.contains_key(&provider.provider_id))
            .cloned()
            .collect()
    }

    /// Resolve one provider-owned operation from the current atomic register
    /// generation. Identity-only seed entries never participate in dispatch.
    pub fn resolve_route(
        &self,
        language_id: &str,
        operation: &str,
    ) -> Result<(String, CompiledProviderRoute), String> {
        let state = self.state.load();
        let mut matches = state
            .snapshot
            .providers
            .iter()
            .filter(|provider| provider.language_id == language_id)
            .filter_map(|provider| {
                state
                    .routes
                    .get(&provider.provider_id)
                    .map(|routes| (provider.provider_id.as_str(), routes.iter()))
            })
            .flat_map(|(provider_id, routes)| {
                routes
                    .filter(move |route| route.spec().operation == operation)
                    .map(move |route| (provider_id.to_owned(), route.clone()))
            });
        let resolved = matches.next().ok_or_else(|| {
            format!(
                "state=route-missing reasonKind=operation-not-in-installed-capability languageId={language_id} operation={operation}"
            )
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "state=route-ambiguous reasonKind=multiple-installed-provider-routes languageId={language_id} operation={operation}"
            ));
        }
        Ok(resolved)
    }

    #[tracing::instrument(name = "asp.runtime.provider_register.apply", skip_all)]
    pub async fn apply(
        &self,
        request: ProviderRegisterRequest,
    ) -> Result<ProviderRegisterResponse, String> {
        let operation = register_operation_label(&request.request);
        tracing::debug!(
            operation,
            expected_generation = request.expected_generation,
            "provider register request received"
        );
        request.validate()?;
        if matches!(request.request, ProviderRegisterOperation::List) {
            return Ok(snapshot_response(self.snapshot()));
        }
        self.apply_mutation(operation, request).await
    }

    async fn apply_mutation(
        &self,
        operation: &'static str,
        request: ProviderRegisterRequest,
    ) -> Result<ProviderRegisterResponse, String> {
        let expected_generation = request
            .expected_generation
            .ok_or_else(|| "provider register mutations require expectedGeneration".to_owned())?;
        let _writer = self.writer.lock().await;
        let current = self.state.load_full();
        if expected_generation != current.snapshot.generation {
            tracing::warn!(
                operation,
                expected_generation,
                current_generation = current.snapshot.generation,
                "provider register generation conflict"
            );
            return Ok(generation_conflict_response(current.snapshot.generation));
        }

        let mut providers = current
            .snapshot
            .providers
            .iter()
            .cloned()
            .map(|provider| (provider.provider_id.clone(), provider))
            .collect::<BTreeMap<_, _>>();
        match request.request {
            ProviderRegisterOperation::Initialize { providers: initial } => {
                if current.snapshot.generation != 0 {
                    return Ok(generation_conflict_response(current.snapshot.generation));
                }
                providers = initial
                    .into_iter()
                    .map(|provider| (provider.provider_id.clone(), provider))
                    .collect();
            }
            ProviderRegisterOperation::Register { provider } => {
                if let Some(identity) = self.identity_constraints.get(&provider.provider_id)
                    && identity.language_id != provider.language_id
                {
                    return Ok(rejected_response(
                        "provider-identity-drift",
                        format!(
                            "provider `{}` is admitted for language `{}`, not `{}`",
                            provider.provider_id, identity.language_id, provider.language_id
                        ),
                    ));
                }
                providers.insert(provider.provider_id.clone(), provider);
            }
            ProviderRegisterOperation::Unregister { provider_id } => {
                if let Some(identity) = self.identity_constraints.get(&provider_id) {
                    providers.insert(provider_id, identity.clone());
                } else {
                    providers.remove(&provider_id);
                }
            }
            ProviderRegisterOperation::List => {
                unreachable!("list returned before writer admission")
            }
        }

        let providers = providers.into_values().collect::<Vec<_>>();
        if providers == current.snapshot.providers {
            return Ok(snapshot_response(Arc::clone(&current.snapshot)));
        }
        let generation = current
            .snapshot
            .generation
            .checked_add(1)
            .ok_or_else(|| "provider register generation overflow".to_owned())?;
        let next = Arc::new(build_state(generation, providers)?);
        if let Some(store_path) = &self.store_path {
            persist_external_providers(
                store_path,
                next.snapshot
                    .providers
                    .iter()
                    .filter(|provider| next.routes.contains_key(&provider.provider_id))
                    .cloned()
                    .collect(),
            )
            .await?;
        }
        self.state.store(Arc::clone(&next));
        tracing::info!(
            operation,
            generation,
            provider_count = next.snapshot.providers.len(),
            route_provider_count = next.routes.len(),
            "provider register generation published"
        );
        Ok(snapshot_response(Arc::clone(&next.snapshot)))
    }
}

fn runtime_contract_schemas<'a>(
    provider: &'a ProviderRegistrationDocument,
    operation: &str,
) -> Option<(&'a str, &'a str)> {
    provider
        .registration
        .get("runtimeContract")?
        .get("operations")?
        .as_array()?
        .iter()
        .find(|candidate| {
            candidate
                .get("operation")
                .and_then(serde_json::Value::as_str)
                == Some(operation)
        })
        .and_then(|candidate| {
            Some((
                candidate.get("requestSchemaId")?.as_str()?,
                candidate.get("responseSchemaId")?.as_str()?,
            ))
        })
}

fn client_parameter_type(
    value: agent_semantic_provider_protocol::ProviderRouteValueType,
) -> ClientParameterType {
    use agent_semantic_provider_protocol::ProviderRouteValueType as Source;
    match value {
        Source::String => ClientParameterType::String,
        Source::WorkspaceRelativePath => ClientParameterType::WorkspaceRelativePath,
        Source::StructuralSelector => ClientParameterType::StructuralSelector,
        Source::Presentation => ClientParameterType::Presentation,
        Source::Boolean => ClientParameterType::Boolean,
        Source::UnsignedInteger => ClientParameterType::UnsignedInteger,
        Source::Json => ClientParameterType::Json,
    }
}

fn client_parameter_cardinality(
    value: agent_semantic_provider_protocol::ProviderRouteCardinality,
) -> ClientParameterCardinality {
    use agent_semantic_provider_protocol::ProviderRouteCardinality as Source;
    match value {
        Source::Required => ClientParameterCardinality::Required,
        Source::Optional => ClientParameterCardinality::Optional,
        Source::Many => ClientParameterCardinality::Many,
    }
}

fn client_parameter_source(
    value: agent_semantic_provider_protocol::ProviderRouteInputSource,
) -> ClientParameterSource {
    use agent_semantic_provider_protocol::ProviderRouteInputSource as Source;
    match value {
        Source::Request => ClientParameterSource::Request,
        Source::RuntimeContext => ClientParameterSource::RuntimeContext,
    }
}

fn register_operation_label(operation: &ProviderRegisterOperation) -> &'static str {
    match operation {
        ProviderRegisterOperation::Initialize { .. } => "initialize",
        ProviderRegisterOperation::Register { .. } => "register",
        ProviderRegisterOperation::Unregister { .. } => "unregister",
        ProviderRegisterOperation::List => "list",
    }
}

fn build_snapshot(
    generation: u64,
    providers: Vec<ProviderRegistrationDocument>,
) -> ProviderRegisterSnapshot {
    let encoded = serde_json::to_vec(&providers).expect("provider register serialization");
    let digest = format!("sha256:{:x}", Sha256::digest(encoded));
    ProviderRegisterSnapshot {
        generation,
        digest,
        providers,
    }
}

fn build_state(
    generation: u64,
    providers: Vec<ProviderRegistrationDocument>,
) -> Result<RuntimeProviderRegisterState, String> {
    let snapshot = Arc::new(build_snapshot(generation, providers));
    snapshot.validate()?;
    let mut routes = BTreeMap::new();
    for provider in &snapshot.providers {
        if provider.registration.get("routes").is_some() {
            routes.insert(
                provider.provider_id.clone(),
                Arc::from(provider.compiled_routes()?),
            );
        }
    }
    Ok(RuntimeProviderRegisterState { snapshot, routes })
}

fn validate_provider_set(providers: &[ProviderRegistrationDocument]) -> Result<(), String> {
    let snapshot = build_snapshot(1, providers.to_vec());
    snapshot.validate()
}

fn snapshot_response(snapshot: Arc<ProviderRegisterSnapshot>) -> ProviderRegisterResponse {
    ProviderRegisterResponse {
        schema_id: PROVIDER_REGISTER_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
        result: ProviderRegisterResult::Snapshot {
            snapshot: Arc::unwrap_or_clone(snapshot),
        },
    }
}

fn generation_conflict_response(actual_generation: u64) -> ProviderRegisterResponse {
    ProviderRegisterResponse {
        schema_id: PROVIDER_REGISTER_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
        result: ProviderRegisterResult::GenerationConflict { actual_generation },
    }
}

fn rejected_response(reason_kind: &str, message: String) -> ProviderRegisterResponse {
    ProviderRegisterResponse {
        schema_id: PROVIDER_REGISTER_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
        result: ProviderRegisterResult::Rejected {
            reason_kind: reason_kind.to_owned(),
            message,
        },
    }
}

async fn read_external_providers(
    store_path: &Path,
) -> Result<Vec<ProviderRegistrationDocument>, String> {
    let source = match tokio::fs::read(store_path).await {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to read provider register state {}: {error}",
                store_path.display()
            ));
        }
    };
    let state: PersistedProviderRegisterState =
        serde_json::from_slice(&source).map_err(|error| {
            format!(
                "failed to parse provider register state {}: {error}",
                store_path.display()
            )
        })?;
    if state.schema_id != PROVIDER_REGISTER_STATE_SCHEMA_ID
        || state.schema_version != PROVIDER_REGISTER_SCHEMA_VERSION
    {
        return Err(format!(
            "provider register state schema mismatch at {}",
            store_path.display()
        ));
    }
    validate_provider_set(&state.providers)?;
    Ok(state.providers)
}

async fn persist_external_providers(
    store_path: &Path,
    providers: Vec<ProviderRegistrationDocument>,
) -> Result<(), String> {
    let parent = store_path
        .parent()
        .ok_or_else(|| "provider register state path has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        format!(
            "failed to create provider register state directory {}: {error}",
            parent.display()
        )
    })?;
    let state = PersistedProviderRegisterState {
        schema_id: PROVIDER_REGISTER_STATE_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_REGISTER_SCHEMA_VERSION.to_owned(),
        providers,
    };
    let encoded = serde_json::to_vec(&state)
        .map_err(|error| format!("failed to encode provider register state: {error}"))?;
    let temporary = store_path.with_extension(format!("tmp-{}", std::process::id()));
    tokio::fs::write(&temporary, encoded)
        .await
        .map_err(|error| {
            format!(
                "failed to write provider register state {}: {error}",
                temporary.display()
            )
        })?;
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&temporary)
        .await
        .map_err(|error| format!("failed to reopen provider register state: {error}"))?;
    file.sync_all()
        .await
        .map_err(|error| format!("failed to sync provider register state: {error}"))?;
    tokio::fs::rename(&temporary, store_path)
        .await
        .map_err(|error| {
            format!(
                "failed to publish provider register state {}: {error}",
                store_path.display()
            )
        })
}
