//! Semantic validation for provider-scoped incremental search generation packets.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use serde_json::Map;
use serde_json::Value;

/// Cross-field validation failure for an incremental search generation packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncrementalSearchGenerationValidationError {
    reason_kind: &'static str,
    message: String,
}

impl IncrementalSearchGenerationValidationError {
    /// Stable machine-readable classification for the failed invariant.
    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for IncrementalSearchGenerationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for IncrementalSearchGenerationValidationError {}

/// Validate semantic invariants that JSON Schema cannot express across fields.
pub fn validate_incremental_search_generation_v1(
    packet: &Value,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    let packet = required_object(packet, "packet")?;
    validate_identity(packet)?;
    let completeness = validate_completeness(packet)?;
    validate_budget_counters(packet, completeness)?;
    validate_completion_state(packet, completeness)?;
    if required_text(packet, "operation")? == "treesitter-query" {
        validate_tree_sitter_packet(packet, completeness)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Completeness<'a> {
    state: &'a str,
    processed: u64,
    dirty: u64,
    remaining: u64,
    budget: u64,
    remaining_kind: &'a str,
}

fn validate_identity(
    packet: &Map<String, Value>,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    require_equal_text(
        required_text(packet, "schemaId")?,
        "agent.semantic-protocols.incremental-search-generation",
        "schema-identity-drift",
        "schemaId",
    )?;
    require_equal_text(
        required_text(packet, "schemaVersion")?,
        "1",
        "schema-identity-drift",
        "schemaVersion",
    )?;
    Ok(())
}

fn validate_completeness<'a>(
    packet: &'a Map<String, Value>,
) -> Result<Completeness<'a>, IncrementalSearchGenerationValidationError> {
    let completeness = required_object_field(packet, "completeness")?;
    Ok(Completeness {
        state: required_text(completeness, "state")?,
        processed: required_u64(completeness, "processedOwnerCount")?,
        dirty: required_u64(completeness, "dirtyOwnerCount")?,
        remaining: required_u64(completeness, "remainingOwnerCount")?,
        budget: required_u64(completeness, "incrementalBudget")?,
        remaining_kind: required_text(completeness, "remainingCountKind")?,
    })
}

fn validate_budget_counters(
    packet: &Map<String, Value>,
    completeness: Completeness<'_>,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    if completeness.processed > completeness.budget || completeness.processed > completeness.dirty {
        return invalid(
            "incremental-budget-exceeded",
            "processedOwnerCount exceeds incrementalBudget or dirtyOwnerCount",
        );
    }
    let counters = required_object_field(packet, "counters")?;
    for field in [
        "sourceByteReads",
        "providerParses",
        "queryCacheWrites",
        "completeOwnerRefreshes",
        "ownerIndexWrites",
    ] {
        if required_u64(counters, field)? > completeness.processed {
            return invalid(
                "incremental-counter-exceeded",
                format!("{field} exceeds processedOwnerCount"),
            );
        }
    }
    Ok(())
}

fn validate_completion_state(
    packet: &Map<String, Value>,
    completeness: Completeness<'_>,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    let continuation = packet
        .get("continuation")
        .ok_or_else(|| validation_error("missing-field", "packet is missing continuation"))?;
    match completeness.state {
        "complete"
            if completeness.remaining == 0
                && completeness.remaining_kind == "exact"
                && continuation.is_null() =>
        {
            Ok(())
        }
        "complete" => invalid(
            "invalid-complete-state",
            "complete requires exact zero remaining owners and no continuation",
        ),
        "partial" if continuation.is_object() => Ok(()),
        "partial" => invalid(
            "invalid-partial-state",
            "partial requires a typed continuation",
        ),
        state => invalid(
            "invalid-completeness-state",
            format!("unsupported completeness state {state}"),
        ),
    }
}

fn validate_tree_sitter_packet(
    packet: &Map<String, Value>,
    completeness: Completeness<'_>,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    let query = required_object_field(packet, "query")?;
    let query_digest = required_text(query, "queryDigest")?;
    let captures = text_set(required_array(query, "captureNames")?, "captureNames")?;
    let inventory = required_object_field(packet, "inventory")?;
    validate_inventory(inventory, completeness)?;
    let results = validate_query_owner_results(packet, query_digest, completeness)?;
    validate_capture_projections(packet, query_digest, &captures, &results)?;
    validate_tree_sitter_counters(packet, &results)?;
    validate_continuation_identity(packet, query_digest, inventory, completeness)?;
    Ok(())
}

#[derive(Default)]
struct QueryOwnerEvidence {
    expected_captures: BTreeMap<String, u64>,
    processed_count: u64,
    refresh_count: u64,
    cached_count: u64,
}

fn validate_query_owner_results(
    packet: &Map<String, Value>,
    query_digest: &str,
    completeness: Completeness<'_>,
) -> Result<QueryOwnerEvidence, IncrementalSearchGenerationValidationError> {
    let mut evidence = QueryOwnerEvidence::default();
    for result in required_array(packet, "queryOwnerResults")? {
        let result = required_object(result, "queryOwnerResults[]")?;
        require_equal_text(
            required_text(result, "queryDigest")?,
            query_digest,
            "query-cache-key-drift",
            "queryOwnerResults.queryDigest",
        )?;
        let owner_path = required_text(result, "ownerPath")?;
        let owner_digest = required_text(result, "ownerContentDigest")?;
        let key = query_owner_key(owner_path, owner_digest, query_digest);
        if evidence
            .expected_captures
            .insert(key, required_u64(result, "captureCount")?)
            .is_some()
        {
            return invalid(
                "duplicate-query-owner-cache-key",
                "queryOwnerResults contains a duplicate cache identity",
            );
        }
        evidence.refresh_count += required_u64(result, "completeOwnerRefreshCount")?;
        match required_text(result, "state")? {
            "processed" => evidence.processed_count += 1,
            "cached" => evidence.cached_count += 1,
            state => {
                return invalid(
                    "invalid-query-owner-state",
                    format!("unsupported query owner state {state}"),
                );
            }
        }
    }
    if evidence.processed_count != completeness.processed {
        return invalid(
            "processed-owner-count-drift",
            "processed query owner results do not match processedOwnerCount",
        );
    }
    Ok(evidence)
}

fn validate_capture_projections(
    packet: &Map<String, Value>,
    query_digest: &str,
    captures: &BTreeSet<String>,
    results: &QueryOwnerEvidence,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    let language_id = required_text(packet, "languageId")?;
    let mut observed = BTreeMap::<String, u64>::new();
    for projection in required_array(packet, "projections")? {
        let projection = required_object(projection, "projections[]")?;
        require_equal_text(
            required_text(projection, "queryDigest")?,
            query_digest,
            "query-cache-key-drift",
            "projection.queryDigest",
        )?;
        let owner_path = required_text(projection, "ownerPath")?;
        let owner_digest = required_text(projection, "sourceContentDigest")?;
        let key = query_owner_key(owner_path, owner_digest, query_digest);
        if !results.expected_captures.contains_key(&key) {
            return invalid(
                "projection-owner-cache-key-missing",
                "capture projection has no query-owner result",
            );
        }
        let capture_name = required_text(projection, "captureName")?;
        if !captures.contains(capture_name) {
            return invalid(
                "capture-name-drift",
                format!("capture {capture_name} is not declared by the query"),
            );
        }
        validate_projection_identity(projection, language_id, owner_path)?;
        *observed.entry(key).or_default() += 1;
    }
    for (key, expected) in &results.expected_captures {
        if observed.get(key).copied().unwrap_or_default() != *expected {
            return invalid(
                "capture-count-drift",
                "query-owner captureCount does not match admitted projections",
            );
        }
    }
    Ok(())
}

fn validate_projection_identity(
    projection: &Map<String, Value>,
    language_id: &str,
    owner_path: &str,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    let capture_start = required_u64(projection, "sourceByteStart")?;
    let capture_end = required_u64(projection, "sourceByteEnd")?;
    let item_start = required_u64(projection, "itemSourceByteStart")?;
    let item_end = required_u64(projection, "itemSourceByteEnd")?;
    if item_start > capture_start || capture_start >= capture_end || capture_end > item_end {
        return invalid(
            "capture-item-span-drift",
            "capture span is not contained by a non-empty item span",
        );
    }
    let selector_prefix = format!("{language_id}://{owner_path}#item/");
    if !required_text(projection, "structuralSelector")?.starts_with(&selector_prefix) {
        return invalid(
            "canonical-selector-drift",
            "structural selector does not identify the projected owner item",
        );
    }
    if required_text(projection, "signature")?.is_empty() {
        return invalid("empty-signature", "projection signature is empty");
    }
    Ok(())
}

fn validate_tree_sitter_counters(
    packet: &Map<String, Value>,
    results: &QueryOwnerEvidence,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    let counters = required_object_field(packet, "counters")?;
    if required_u64(counters, "queryCacheWrites")? != results.processed_count
        || required_u64(counters, "completeOwnerRefreshes")? != results.refresh_count
    {
        return invalid(
            "query-cache-counter-drift",
            "query cache or complete-owner refresh counters do not match owner results",
        );
    }
    if results.processed_count == 0 {
        for field in [
            "sourceByteReads",
            "providerParses",
            "queryCacheWrites",
            "completeOwnerRefreshes",
            "ownerIndexWrites",
        ] {
            if required_u64(counters, field)? != 0 {
                return invalid(
                    "warm-query-side-effect",
                    format!("warm query reports non-zero {field}"),
                );
            }
        }
    }
    Ok(())
}

fn validate_inventory(
    inventory: &Map<String, Value>,
    completeness: Completeness<'_>,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    match required_text(inventory, "state")? {
        "exact" if completeness.remaining_kind == "exact" => {}
        "known" if completeness.remaining_kind == "known-lower-bound" => {}
        _ => {
            return invalid(
                "inventory-count-kind-drift",
                "inventory state and remainingCountKind disagree",
            );
        }
    }
    if completeness.state == "complete" && required_text(inventory, "state")? != "exact" {
        return invalid(
            "known-inventory-complete",
            "known inventory cannot close enumeration or absence",
        );
    }
    Ok(())
}

fn validate_continuation_identity(
    packet: &Map<String, Value>,
    query_digest: &str,
    inventory: &Map<String, Value>,
    completeness: Completeness<'_>,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    if completeness.state == "complete" {
        return Ok(());
    }
    let continuation = required_object_field(packet, "continuation")?;
    for (field, expected) in [
        (
            "providerWorkspaceIdentityDigest",
            required_text(packet, "providerWorkspaceIdentityDigest")?,
        ),
        ("queryDigest", query_digest),
        ("inventoryState", required_text(inventory, "state")?),
        ("remainingCountKind", completeness.remaining_kind),
    ] {
        require_equal_text(
            required_text(continuation, field)?,
            expected,
            "continuation-identity-drift",
            field,
        )?;
    }
    let expected_generation = packet
        .get("generationAfter")
        .and_then(Value::as_object)
        .and_then(|generation| generation.get("rootDigest"))
        .and_then(Value::as_str);
    let continued_generation = continuation
        .get("generationRootDigest")
        .and_then(Value::as_str);
    if continued_generation != expected_generation {
        return invalid(
            "continuation-generation-drift",
            "continuation generation does not match generationAfter",
        );
    }
    Ok(())
}

fn query_owner_key(owner_path: &str, owner_digest: &str, query_digest: &str) -> String {
    format!("{owner_path}\0{owner_digest}\0{query_digest}")
}

fn text_set(
    values: &[Value],
    field: &str,
) -> Result<BTreeSet<String>, IncrementalSearchGenerationValidationError> {
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                validation_error(
                    "invalid-field-type",
                    format!("{field} must contain strings"),
                )
            })
        })
        .collect()
}

fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, IncrementalSearchGenerationValidationError> {
    value
        .as_object()
        .ok_or_else(|| validation_error("invalid-field-type", format!("{field} must be an object")))
}

fn required_object_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, IncrementalSearchGenerationValidationError> {
    required_object(
        object.get(field).ok_or_else(|| {
            validation_error("missing-field", format!("packet is missing {field}"))
        })?,
        field,
    )
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a [Value], IncrementalSearchGenerationValidationError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| validation_error("invalid-field-type", format!("{field} must be an array")))
}

fn required_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, IncrementalSearchGenerationValidationError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| validation_error("invalid-field-type", format!("{field} must be a string")))
}

fn required_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<u64, IncrementalSearchGenerationValidationError> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        validation_error(
            "invalid-field-type",
            format!("{field} must be a non-negative integer"),
        )
    })
}

fn require_equal_text(
    actual: &str,
    expected: &str,
    reason_kind: &'static str,
    field: &str,
) -> Result<(), IncrementalSearchGenerationValidationError> {
    if actual == expected {
        Ok(())
    } else {
        invalid(
            reason_kind,
            format!("{field} expected {expected}, got {actual}"),
        )
    }
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, IncrementalSearchGenerationValidationError> {
    Err(validation_error(reason_kind, message))
}

fn validation_error(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> IncrementalSearchGenerationValidationError {
    IncrementalSearchGenerationValidationError {
        reason_kind,
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "../tests/unit/incremental_search_generation.rs"]
mod tests;
