//! Bounded Tokio-stream fan-in for the Runtime search data plane.

use std::{collections::BTreeSet, pin::Pin};

use agent_semantic_search_projection::{
    RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_ID, RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_VERSION,
    ResidentSearchReadyResult, ResidentSearchWorkCounters, RuntimeProviderSearchReceipt,
};
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt, StreamMap, wrappers::ReceiverStream};

pub const RUNTIME_SEARCH_SOURCE_CAPACITY: usize = 32;
pub const RUNTIME_SEARCH_SOURCE_LIMIT: usize = 64;

pub type RuntimeSearchResult = Result<ResidentSearchReadyResult, String>;
type RuntimeSearchResultStream = Pin<Box<dyn Stream<Item = RuntimeSearchResult> + Send>>;

pub struct RuntimeSearchSource {
    source_id: String,
    stream: RuntimeSearchResultStream,
}

impl RuntimeSearchSource {
    pub fn once(source_id: impl Into<String>, result: ResidentSearchReadyResult) -> Self {
        Self {
            source_id: source_id.into(),
            stream: Box::pin(tokio_stream::once(Ok(result))),
        }
    }
}

pub fn bounded_runtime_search_source(
    source_id: impl Into<String>,
) -> (mpsc::Sender<RuntimeSearchResult>, RuntimeSearchSource) {
    let (sender, receiver) = mpsc::channel(RUNTIME_SEARCH_SOURCE_CAPACITY);
    (
        sender,
        RuntimeSearchSource {
            source_id: source_id.into(),
            stream: Box::pin(ReceiverStream::new(receiver)),
        },
    )
}

pub async fn build_runtime_provider_search_receipt(
    operation_id: String,
    language_id: agent_semantic_client_core::LanguageId,
    sources: Vec<RuntimeSearchSource>,
    resident_read_elapsed_micros: u64,
    parser_owned_selector_pairs: Vec<(String, String)>,
) -> Result<RuntimeProviderSearchReceipt, String> {
    let started = std::time::Instant::now();
    let mut fan_in = admitted_source_map(sources)?;
    let mut authority: Option<SearchAuthority> = None;
    let mut candidate_count = 0usize;
    let mut selectors = BTreeSet::new();
    let mut owner_paths = BTreeSet::new();
    let mut work_counters = ResidentSearchWorkCounters::default();

    while let Some((source_id, result)) = fan_in.next().await {
        let result = result.map_err(|error| format!("search source `{source_id}`: {error}"))?;
        result.validate()?;
        let result_authority = SearchAuthority::from(&result);
        match &authority {
            Some(current) if current != &result_authority => {
                return Err(format!(
                    "search source `{source_id}` crossed Runtime generation authority"
                ));
            }
            None => authority = Some(result_authority),
            _ => {}
        }
        candidate_count = candidate_count.saturating_add(result.hits.len());
        for hit in result.hits {
            selectors.extend(hit.selector);
            owner_paths.insert(hit.owner_path);
        }
        accumulate_work_counters(&mut work_counters, result.work_counters);
    }

    let authority =
        authority.ok_or_else(|| "runtime search requires one source result".to_owned())?;
    for (selector, owner_path) in parser_owned_selector_pairs {
        selectors.insert(selector);
        owner_paths.insert(owner_path);
    }
    let service_elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let receipt = RuntimeProviderSearchReceipt {
        schema_id: RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_VERSION.to_owned(),
        operation_id,
        status: if candidate_count == 0 {
            "no-matches".to_owned()
        } else {
            "matches".to_owned()
        },
        language_id: language_id.to_string(),
        generation_digest: authority.generation_digest,
        root_digest: authority.root_digest,
        provider_digest: authority.provider_digest,
        index_artifact_digest: authority.index_artifact_digest,
        candidate_count,
        selectors: selectors.into_iter().collect(),
        owner_paths: owner_paths.into_iter().collect(),
        resident_read_elapsed_micros,
        service_elapsed_micros,
        elapsed_micros: resident_read_elapsed_micros.saturating_add(service_elapsed_micros),
        work_counters,
    };
    receipt.validate()?;
    Ok(receipt)
}

fn admitted_source_map(
    sources: Vec<RuntimeSearchSource>,
) -> Result<StreamMap<String, RuntimeSearchResultStream>, String> {
    if sources.is_empty() || sources.len() > RUNTIME_SEARCH_SOURCE_LIMIT {
        return Err(format!(
            "runtime search source count must be in 1..={RUNTIME_SEARCH_SOURCE_LIMIT}"
        ));
    }
    let mut fan_in = StreamMap::new();
    for source in sources {
        if source.source_id.is_empty() || fan_in.contains_key(&source.source_id) {
            return Err("runtime search source ids must be non-empty and unique".to_owned());
        }
        fan_in.insert(source.source_id, source.stream);
    }
    Ok(fan_in)
}

fn accumulate_work_counters(
    total: &mut ResidentSearchWorkCounters,
    next: ResidentSearchWorkCounters,
) {
    total.database_read_count = total
        .database_read_count
        .saturating_add(next.database_read_count);
    total.filesystem_read_count = total
        .filesystem_read_count
        .saturating_add(next.filesystem_read_count);
    total.provider_process_count = total
        .provider_process_count
        .saturating_add(next.provider_process_count);
    total.socket_operation_count = total
        .socket_operation_count
        .saturating_add(next.socket_operation_count);
    total.scheduler_task_count = total
        .scheduler_task_count
        .saturating_add(next.scheduler_task_count);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchAuthority {
    generation_digest: String,
    root_digest: String,
    provider_digest: String,
    index_artifact_digest: String,
}

impl From<&ResidentSearchReadyResult> for SearchAuthority {
    fn from(result: &ResidentSearchReadyResult) -> Self {
        Self {
            generation_digest: result.generation_digest.clone(),
            root_digest: result.root_digest.clone(),
            provider_digest: result.provider_digest.clone(),
            index_artifact_digest: result.index_artifact_digest.clone(),
        }
    }
}
