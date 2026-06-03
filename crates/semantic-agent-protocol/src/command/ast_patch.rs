//! Verification receipts for `semantic-agent-protocol ast-patch`.

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

const PACKET_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-ast-patch";
const RECEIPT_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-ast-patch-receipt";
const AST_PATCH_PROTOCOL_ID: &str = "agent.semantic-protocols.ast-patch";
const SUPPORTED_OPERATIONS: &[&str] = &[
    "append_to_block",
    "insert_before_statement",
    "insert_after_statement",
    "replace_statement",
    "replace_expression",
    "replace_call_arg",
    "insert_import",
    "remove_import",
    "remove_statement",
    "remove_item",
    "replace_item",
];
const LARGE_MECHANICAL_OPERATIONS: &[&str] = &[
    "insert_import",
    "remove_import",
    "remove_statement",
    "remove_item",
    "replace_statement",
    "replace_item",
];

pub(crate) fn run_ast_patch_command(args: &[String]) -> Result<(), String> {
    let request = AstPatchRequest::parse(args)?;
    let packet = read_packet(&request.packet_path)?;
    let receipt = receipt_for_packet(&packet, request.mode);
    let output = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("failed to serialize ast patch receipt: {error}"))?;
    println!("{output}");
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum AstPatchMode {
    Verify,
    DryRun,
}

impl AstPatchMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::DryRun => "dry-run",
        }
    }
}

struct AstPatchRequest {
    mode: AstPatchMode,
    packet_path: PathBuf,
}

impl AstPatchRequest {
    fn parse(args: &[String]) -> Result<Self, String> {
        let Some(mode) = args.first().map(String::as_str) else {
            return Err(usage());
        };
        if mode == "apply" {
            return Err(
                "ast-patch apply is unavailable in the Codex adapter; use dry-run and Codex apply_patch"
                    .to_string(),
            );
        }
        let mode = match mode {
            "verify" => AstPatchMode::Verify,
            "dry-run" => AstPatchMode::DryRun,
            _ => return Err(usage()),
        };
        let packet_path = flag_value(&args[1..], "--packet")
            .ok_or_else(|| "missing required --packet <path-or->".to_string())?;
        Ok(Self {
            mode,
            packet_path: PathBuf::from(packet_path),
        })
    }
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}

fn read_packet(path: &PathBuf) -> Result<Value, String> {
    let mut contents = String::new();
    if path.as_os_str() == "-" {
        io::stdin()
            .read_to_string(&mut contents)
            .map_err(|error| format!("failed to read ast patch packet from stdin: {error}"))?;
    } else {
        contents = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    }
    serde_json::from_str(&contents)
        .map_err(|error| format!("invalid ast patch packet JSON: {error}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AstPatchReceipt {
    pub(super) schema_id: &'static str,
    pub(super) schema_version: &'static str,
    pub(super) protocol_id: &'static str,
    pub(super) protocol_version: &'static str,
    pub(super) status: &'static str,
    pub(super) mode: &'static str,
    pub(super) capability: &'static str,
    pub(super) mutation_available: bool,
    pub(super) language_id: Option<String>,
    pub(super) target: ReceiptTarget,
    pub(super) operation: Option<String>,
    pub(super) supported_operations: &'static [&'static str],
    pub(super) mechanical_edit_plan: Option<MechanicalEditPlan>,
    pub(super) verification: Vec<&'static str>,
    pub(super) failure_kind: Option<&'static str>,
    pub(super) failures: Vec<String>,
    pub(super) next: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiptTarget {
    pub(super) owner_path: Option<String>,
    pub(super) locator: Option<String>,
    pub(super) read: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MechanicalEditPlan {
    pub(super) kind: &'static str,
    pub(super) operation: String,
    pub(super) target_read: String,
    pub(super) estimated_edits: u64,
    pub(super) max_edits: u64,
    pub(super) safe_for_large_change: bool,
    pub(super) mutation_available: bool,
    pub(super) requires_codex_apply_patch: bool,
    pub(super) changed_ranges: Vec<String>,
    pub(super) notes: Vec<String>,
}

pub(super) fn receipt_for_packet(packet: &Value, mode: AstPatchMode) -> AstPatchReceipt {
    let mut verification = vec!["packet-parsed", "codex-mutation-disabled"];
    let mut failures = Vec::new();

    if packet.get("schemaId").and_then(Value::as_str) == Some(PACKET_SCHEMA_ID) {
        verification.push("schema-id-valid");
    } else {
        failures.push(format!("schemaId must be {PACKET_SCHEMA_ID}"));
    }
    if packet.get("schemaVersion").and_then(Value::as_str) == Some("1") {
        verification.push("schema-version-valid");
    } else {
        failures.push("schemaVersion must be 1".to_string());
    }
    if packet.get("protocolId").and_then(Value::as_str) == Some(AST_PATCH_PROTOCOL_ID) {
        verification.push("protocol-id-valid");
    } else {
        failures.push(format!("protocolId must be {AST_PATCH_PROTOCOL_ID}"));
    }
    if packet.get("protocolVersion").and_then(Value::as_str) == Some("1") {
        verification.push("protocol-version-valid");
    } else {
        failures.push("protocolVersion must be 1".to_string());
    }

    let target = packet.get("target").unwrap_or(&Value::Null);
    let receipt_target = ReceiptTarget {
        owner_path: string_field(target, "ownerPath"),
        locator: string_field(target, "locator"),
        read: string_field(target, "read"),
    };
    if receipt_target.owner_path.is_some() {
        verification.push("target-owner-present");
    } else {
        failures.push("target.ownerPath is required".to_string());
    }
    if receipt_target.locator.is_some() {
        verification.push("target-locator-present");
    } else {
        failures.push("target.locator is required".to_string());
    }
    match receipt_target.read.as_deref() {
        Some(read) if is_source_locator(read) => verification.push("target-read-valid"),
        Some(_) => failures.push("target.read must use path:start:end source locator".to_string()),
        None => failures.push("target.read is required".to_string()),
    }

    let operation = packet.get("operation").unwrap_or(&Value::Null);
    let op = string_field(operation, "op");
    let max_edits = integer_field(operation, "maxEdits").unwrap_or(1);
    let allow_large_mechanical_edit =
        bool_field(operation, "allowLargeMechanicalEdit").unwrap_or(false);
    match op.as_deref() {
        Some(operation_name) if SUPPORTED_OPERATIONS.contains(&operation_name) => {
            verification.push("operation-supported");
            if operation_requires_snippet(operation_name) {
                if string_field(operation, "snippet").is_some() {
                    verification.push("snippet-present");
                } else {
                    failures.push(format!(
                        "operation {operation_name} requires operation.snippet"
                    ));
                }
            }
            if allow_large_mechanical_edit {
                verification.push("large-mechanical-edit-requested");
                if !LARGE_MECHANICAL_OPERATIONS.contains(&operation_name) {
                    failures.push(format!(
                        "operation {operation_name} is not safe for large mechanical edits"
                    ));
                }
                if max_edits < 2 {
                    failures.push(
                        "operation.maxEdits must be at least 2 for large mechanical edits"
                            .to_string(),
                    );
                }
                if string_field(operation, "expectedSnippet").is_some() {
                    verification.push("expected-snippet-present");
                } else {
                    failures.push(
                        "operation.expectedSnippet is required for large mechanical edits"
                            .to_string(),
                    );
                }
                if string_field(operation, "mechanicalKind").is_some() {
                    verification.push("mechanical-kind-present");
                } else {
                    failures.push(
                        "operation.mechanicalKind is required for large mechanical edits"
                            .to_string(),
                    );
                }
            }
        }
        Some(operation_name) => failures.push(format!("unsupported operation {operation_name}")),
        None => failures.push("operation.op is required".to_string()),
    }

    let status = if failures.is_empty() {
        "verified"
    } else {
        "failed"
    };

    let mechanical_edit_plan = if failures.is_empty() {
        op.as_ref().and_then(|operation_name| {
            receipt_target
                .read
                .as_ref()
                .map(|read| MechanicalEditPlan {
                    kind: "codex-dry-run",
                    operation: operation_name.clone(),
                    target_read: read.clone(),
                    estimated_edits: if allow_large_mechanical_edit {
                        max_edits
                    } else {
                        1
                    },
                    max_edits,
                    safe_for_large_change: allow_large_mechanical_edit
                        && LARGE_MECHANICAL_OPERATIONS.contains(&operation_name.as_str()),
                    mutation_available: false,
                    requires_codex_apply_patch: true,
                    changed_ranges: vec![read.clone()],
                    notes: vec![if allow_large_mechanical_edit {
                        "bounded mechanical edit intent verified; provider AST dry-run should estimate exact affected nodes before mutation".to_string()
                    } else {
                        "single-target AST patch intent verified; Codex adapter still delegates mutation to apply_patch".to_string()
                    }],
                })
        })
    } else {
        None
    };
    AstPatchReceipt {
        schema_id: RECEIPT_SCHEMA_ID,
        schema_version: "1",
        protocol_id: AST_PATCH_PROTOCOL_ID,
        protocol_version: "1",
        status,
        mode: mode.as_str(),
        capability: "codex-compact-safety-verifier",
        mutation_available: false,
        language_id: string_field(packet, "languageId"),
        target: receipt_target,
        operation: op,
        supported_operations: SUPPORTED_OPERATIONS,
        mechanical_edit_plan,
        verification,
        failure_kind: (!failures.is_empty()).then_some("invalid-packet"),
        failures,
        next: "Codex adapter: use this receipt to decide whether a provider AST dry-run is needed before Codex apply_patch".to_string(),
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn integer_field(value: &Value, field: &str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64)
}
fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}
fn operation_requires_snippet(operation: &str) -> bool {
    matches!(
        operation,
        "append_to_block"
            | "insert_before_statement"
            | "insert_after_statement"
            | "replace_statement"
            | "replace_expression"
            | "replace_call_arg"
            | "insert_import"
            | "replace_item"
    )
}
fn is_source_locator(read: &str) -> bool {
    let mut parts = read.rsplitn(3, ':');
    let end = parts.next().and_then(|value| value.parse::<usize>().ok());
    let start = parts.next().and_then(|value| value.parse::<usize>().ok());
    let path = parts.next();
    matches!((path, start, end), (Some(path), Some(start), Some(end)) if !path.is_empty() && start > 0 && end >= start)
}

fn usage() -> String {
    "usage: semantic-agent-protocol ast-patch <verify|dry-run> --packet <path-or-> [PROJECT_ROOT]"
        .to_string()
}
