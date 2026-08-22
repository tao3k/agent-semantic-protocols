use agent_semantic_provider_protocol::{
    PROVIDER_REGISTER_RESPONSE_SCHEMA_ID, PROVIDER_REGISTER_SCHEMA_VERSION,
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResponse,
    ProviderRegisterResult, ProviderRegisterSnapshot, ProviderRegistrationDocument,
};
use arc_swap::ArcSwap;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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
    snapshot: ArcSwap<ProviderRegisterSnapshot>,
    writer: tokio::sync::Mutex<()>,
    builtin_provider_ids: BTreeSet<String>,
    store_path: Option<PathBuf>,
}

impl Default for RuntimeProviderRegister {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProviderRegister {
    pub fn new() -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(build_snapshot(0, Vec::new())),
            writer: tokio::sync::Mutex::new(()),
            builtin_provider_ids: BTreeSet::new(),
            store_path: None,
        }
    }

    pub fn from_seed(providers: Vec<ProviderRegistrationDocument>) -> Result<Self, String> {
        validate_provider_set(&providers)?;
        let builtin_provider_ids = providers
            .iter()
            .map(|provider| provider.provider_id.clone())
            .collect();
        Ok(Self {
            snapshot: ArcSwap::from_pointee(build_snapshot(1, providers)),
            writer: tokio::sync::Mutex::new(()),
            builtin_provider_ids,
            store_path: None,
        })
    }

    pub async fn from_seed_with_store(
        builtin_providers: Vec<ProviderRegistrationDocument>,
        store_path: PathBuf,
    ) -> Result<Self, String> {
        let mut register = Self::from_seed(builtin_providers)?;
        let external_providers = read_external_providers(&store_path).await?;
        for provider in &external_providers {
            if register
                .builtin_provider_ids
                .contains(&provider.provider_id)
            {
                return Err(format!(
                    "persisted external provider `{}` conflicts with builtin authority",
                    provider.provider_id
                ));
            }
        }
        let mut providers = register.snapshot().providers.clone();
        providers.extend(external_providers);
        providers.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        validate_provider_set(&providers)?;
        register.snapshot = ArcSwap::from_pointee(build_snapshot(1, providers));
        register.store_path = Some(store_path);
        Ok(register)
    }

    pub fn snapshot(&self) -> Arc<ProviderRegisterSnapshot> {
        self.snapshot.load_full()
    }

    pub async fn apply(
        &self,
        request: ProviderRegisterRequest,
    ) -> Result<ProviderRegisterResponse, String> {
        request.validate()?;
        if matches!(request.request, ProviderRegisterOperation::List) {
            return Ok(snapshot_response(self.snapshot()));
        }

        let expected_generation = request
            .expected_generation
            .ok_or_else(|| "provider register mutations require expectedGeneration".to_owned())?;
        let _writer = self.writer.lock().await;
        let current = self.snapshot.load_full();
        if expected_generation != current.generation {
            return Ok(generation_conflict_response(current.generation));
        }

        let mut providers = current
            .providers
            .iter()
            .cloned()
            .map(|provider| (provider.provider_id.clone(), provider))
            .collect::<BTreeMap<_, _>>();
        match request.request {
            ProviderRegisterOperation::Initialize { providers: initial } => {
                if current.generation != 0 {
                    return Ok(generation_conflict_response(current.generation));
                }
                providers = initial
                    .into_iter()
                    .map(|provider| (provider.provider_id.clone(), provider))
                    .collect();
            }
            ProviderRegisterOperation::Register { provider } => {
                if self.builtin_provider_ids.contains(&provider.provider_id) {
                    return Ok(rejected_response(
                        "builtin-provider-owned",
                        format!(
                            "builtin provider `{}` is owned by the source Provider Register",
                            provider.provider_id
                        ),
                    ));
                }
                providers.insert(provider.provider_id.clone(), provider);
            }
            ProviderRegisterOperation::Unregister { provider_id } => {
                if self.builtin_provider_ids.contains(&provider_id) {
                    return Ok(rejected_response(
                        "builtin-provider-owned",
                        format!(
                            "builtin provider `{provider_id}` is owned by the source Provider Register"
                        ),
                    ));
                }
                providers.remove(&provider_id);
            }
            ProviderRegisterOperation::List => {
                unreachable!("list returned before writer admission")
            }
        }

        let providers = providers.into_values().collect::<Vec<_>>();
        if providers == current.providers {
            return Ok(snapshot_response(current));
        }
        let generation = current
            .generation
            .checked_add(1)
            .ok_or_else(|| "provider register generation overflow".to_owned())?;
        let next = Arc::new(build_snapshot(generation, providers));
        next.validate()?;
        if let Some(store_path) = &self.store_path {
            persist_external_providers(
                store_path,
                next.providers
                    .iter()
                    .filter(|provider| !self.builtin_provider_ids.contains(&provider.provider_id))
                    .cloned()
                    .collect(),
            )
            .await?;
        }
        self.snapshot.store(Arc::clone(&next));
        Ok(snapshot_response(next))
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
