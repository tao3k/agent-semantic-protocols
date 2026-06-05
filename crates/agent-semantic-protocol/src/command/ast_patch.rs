//! Verification receipts for `asp ast-patch`.

use serde::Serialize;
use serde_json::{Map, Value};
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
    "split_owner_items",
];
const LARGE_MECHANICAL_OPERATIONS: &[&str] = &[
    "insert_import",
    "remove_import",
    "remove_statement",
    "remove_item",
    "replace_statement",
    "replace_item",
    "split_owner_items",
];

pub(crate) fn run_ast_patch_command(args: &[String]) -> Result<(), String> {
    if matches!(args.first().map(String::as_str), Some("template" | "draft")) {
        let packet = template_packet_for_args(&args[1..])?;
        let output = serde_json::to_string_pretty(&packet)
            .map_err(|error| format!("failed to serialize ast patch template: {error}"))?;
        println!("{output}");
        return Ok(());
    }
    let request = AstPatchRequest::parse(args)?;
    let packet = read_packet(&request.packet_path)?;
    let receipt = receipt_for_packet(&packet, request.mode);
    let output = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("failed to serialize ast patch receipt: {error}"))?;
    println!("{output}");
    Ok(())
}

struct AstPatchTemplateRequest {
    language_id: String,
    owner_path: String,
    read: String,
    locator: String,
    operation: String,
    snippet: Option<String>,
    expected_snippet: Option<String>,
    mechanical_kind: Option<String>,
    max_edits: Option<u64>,
    allow_large_mechanical_edit: bool,
    project_root: Option<String>,
    provider_id: Option<String>,
    binary: Option<String>,
    namespace: Option<String>,
    item_name: Option<String>,
    item_kind: Option<String>,
    fields: Vec<(String, String)>,
    mutation_source: Option<String>,
    snippet_required: Option<bool>,
    code_in_prompt: Option<bool>,
}

impl AstPatchTemplateRequest {
    fn parse(args: &[String]) -> Result<Self, String> {
        let language_id =
            required_flag(args, "--language").or_else(|_| required_flag(args, "--language-id"))?;
        let owner_path =
            required_flag(args, "--owner").or_else(|_| required_flag(args, "--owner-path"))?;
        let read = required_flag(args, "--read")?;
        let operation = required_flag(args, "--op")?;
        if !SUPPORTED_OPERATIONS.contains(&operation.as_str()) {
            return Err(format!(
                "unsupported --op {operation}; supported operations: {}",
                SUPPORTED_OPERATIONS.join(",")
            ));
        }
        if !is_source_locator(&read) {
            return Err("--read must use path:start:end source locator".to_string());
        }

        let snippet = flag_value(args, "--snippet");
        if operation_requires_snippet(&operation) && snippet.is_none() {
            return Err(format!(
                "--snippet is required for --op {operation}; use exact source context for replacements or inserted source for inserts"
            ));
        }

        let expected_snippet = flag_value(args, "--expected-snippet");
        let mut mechanical_kind = flag_value(args, "--mechanical-kind");
        let max_edits = flag_value(args, "--max-edits")
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| "--max-edits must be an integer".to_string())
            })
            .transpose()?;
        let allow_large_mechanical_edit = flag_present(args, "--allow-large-mechanical-edit");
        let fields = field_values(args)?;
        let mut mutation_source = flag_value(args, "--mutation-source");
        let mut snippet_required = flag_bool_value(args, "--snippet-required")?;
        let mut code_in_prompt = flag_bool_value(args, "--code-in-prompt")?;
        if let Some(source) = mutation_source.as_deref() {
            if !matches!(
                source,
                "provider-native" | "agent-snippet" | "codex-text-fallback"
            ) {
                return Err(
                    "--mutation-source must be provider-native, agent-snippet, or codex-text-fallback"
                        .to_string(),
                );
            }
        }
        if operation == "split_owner_items" {
            if !field_present(&fields, "destinationPath") {
                return Err(
                    "--field destinationPath=<path> is required for --op split_owner_items"
                        .to_string(),
                );
            }
            if !field_present(&fields, "moduleName") {
                return Err(
                    "--field moduleName=<name> is required for --op split_owner_items".to_string(),
                );
            }
            mutation_source.get_or_insert_with(|| "provider-native".to_string());
            snippet_required.get_or_insert(false);
            code_in_prompt.get_or_insert(false);
            mechanical_kind.get_or_insert_with(|| "owner-items".to_string());
        } else if operation_requires_snippet(&operation) {
            mutation_source.get_or_insert_with(|| "agent-snippet".to_string());
            snippet_required.get_or_insert(true);
            code_in_prompt.get_or_insert(true);
        }
        if allow_large_mechanical_edit {
            if !LARGE_MECHANICAL_OPERATIONS.contains(&operation.as_str()) {
                return Err(format!(
                    "--allow-large-mechanical-edit is not supported for --op {operation}"
                ));
            }
            if max_edits.unwrap_or(1) < 2 {
                return Err("--max-edits must be at least 2 for large mechanical edits".to_string());
            }
            if expected_snippet.is_none() {
                return Err(
                    "--expected-snippet is required with --allow-large-mechanical-edit".to_string(),
                );
            }
            if mechanical_kind.is_none() {
                return Err(
                    "--mechanical-kind is required with --allow-large-mechanical-edit".to_string(),
                );
            }
        }

        let (default_provider_id, default_binary) = default_provider_for_language(&language_id);
        let project_root = flag_value(args, "--project-root")
            .or_else(|| trailing_project_root(args))
            .filter(|value| value != "-");
        Ok(Self {
            language_id,
            owner_path,
            locator: flag_value(args, "--locator").unwrap_or_else(|| read.clone()),
            read,
            operation,
            snippet,
            expected_snippet,
            mechanical_kind,
            max_edits,
            allow_large_mechanical_edit,
            project_root,
            provider_id: flag_value(args, "--provider")
                .or_else(|| flag_value(args, "--provider-id"))
                .or(default_provider_id),
            binary: flag_value(args, "--binary").or(default_binary),
            namespace: flag_value(args, "--namespace"),
            item_name: flag_value(args, "--item-name"),
            item_kind: flag_value(args, "--item-kind"),
            fields,
            mutation_source,
            snippet_required,
            code_in_prompt,
        })
    }
}

pub(super) fn template_packet_for_args(args: &[String]) -> Result<Value, String> {
    let request = AstPatchTemplateRequest::parse(args)?;
    Ok(template_packet(request))
}

fn template_packet(request: AstPatchTemplateRequest) -> Value {
    let mut root = Map::new();
    root.insert(
        "schemaId".to_string(),
        Value::String(PACKET_SCHEMA_ID.to_string()),
    );
    root.insert("schemaVersion".to_string(), Value::String("1".to_string()));
    root.insert(
        "protocolId".to_string(),
        Value::String(AST_PATCH_PROTOCOL_ID.to_string()),
    );
    root.insert(
        "protocolVersion".to_string(),
        Value::String("1".to_string()),
    );
    root.insert(
        "languageId".to_string(),
        Value::String(request.language_id.clone()),
    );
    if let Some(provider_id) = request.provider_id {
        root.insert("providerId".to_string(), Value::String(provider_id));
    }
    if let Some(binary) = request.binary {
        root.insert("binary".to_string(), Value::String(binary));
    }
    if let Some(namespace) = request.namespace {
        root.insert("namespace".to_string(), Value::String(namespace));
    }
    if let Some(project_root) = request.project_root.clone() {
        root.insert("projectRoot".to_string(), Value::String(project_root));
    }

    let mut target = Map::new();
    target.insert("ownerPath".to_string(), Value::String(request.owner_path));
    target.insert("locator".to_string(), Value::String(request.locator));
    target.insert("read".to_string(), Value::String(request.read.clone()));
    if let Some((path, line_range)) = location_from_read(&request.read) {
        let mut location = Map::new();
        location.insert("path".to_string(), Value::String(path));
        location.insert("lineRange".to_string(), Value::String(line_range));
        target.insert("location".to_string(), Value::Object(location));
    }
    if let Some(item_name) = request.item_name {
        target.insert("itemName".to_string(), Value::String(item_name));
    }
    if let Some(item_kind) = request.item_kind {
        target.insert("itemKind".to_string(), Value::String(item_kind));
    }
    root.insert("target".to_string(), Value::Object(target));

    let mut operation = Map::new();
    operation.insert("op".to_string(), Value::String(request.operation));
    if let Some(mutation_source) = request.mutation_source {
        operation.insert("mutationSource".to_string(), Value::String(mutation_source));
    }
    if let Some(snippet_required) = request.snippet_required {
        operation.insert("snippetRequired".to_string(), Value::Bool(snippet_required));
    }
    if let Some(code_in_prompt) = request.code_in_prompt {
        operation.insert("codeInPrompt".to_string(), Value::Bool(code_in_prompt));
    }
    if !request.fields.is_empty() {
        let fields = request
            .fields
            .into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect();
        operation.insert("fields".to_string(), Value::Object(fields));
    }
    if let Some(snippet) = request.snippet {
        operation.insert("snippet".to_string(), Value::String(snippet));
    }
    if let Some(expected_snippet) = request.expected_snippet {
        operation.insert(
            "expectedSnippet".to_string(),
            Value::String(expected_snippet),
        );
    }
    if let Some(mechanical_kind) = request.mechanical_kind {
        operation.insert("mechanicalKind".to_string(), Value::String(mechanical_kind));
    }
    if let Some(max_edits) = request.max_edits {
        operation.insert(
            "maxEdits".to_string(),
            Value::Number(serde_json::Number::from(max_edits)),
        );
    }
    if request.allow_large_mechanical_edit {
        operation.insert("allowLargeMechanicalEdit".to_string(), Value::Bool(true));
    }
    root.insert("operation".to_string(), Value::Object(operation));

    let project_root = request.project_root.as_deref().unwrap_or(".");
    root.insert(
        "verificationHints".to_string(),
        Value::Array(vec![
            Value::String("generated-by=asp ast-patch template".to_string()),
            Value::String("exact-read-preimage-required".to_string()),
            Value::String(format!(
                "next=asp {} ast-patch dry-run --packet <semantic-ast-patch.json> {project_root}",
                request.language_id
            )),
        ]),
    );
    Value::Object(root)
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
                "ast-patch apply is unavailable in the Codex adapter; use provider ast-patch apply for provider-native receipts or Codex apply_patch only for explicit text fallback"
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

fn flag_values(args: &[String], flag: &str) -> Vec<String> {
    args.windows(2)
        .filter(|window| window[0] == flag)
        .map(|window| window[1].clone())
        .collect()
}

fn field_values(args: &[String]) -> Result<Vec<(String, String)>, String> {
    flag_values(args, "--field")
        .into_iter()
        .map(|field| {
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| "--field must use key=value".to_string())?;
            if key.trim().is_empty() || value.is_empty() {
                return Err("--field must use non-empty key=value".to_string());
            }
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

fn flag_bool_value(args: &[String], flag: &str) -> Result<Option<bool>, String> {
    flag_value(args, flag)
        .map(|value| match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("{flag} must be true or false")),
        })
        .transpose()
}

fn field_present(fields: &[(String, String)], field: &str) -> bool {
    fields.iter().any(|(key, _)| key == field)
}

fn required_flag(args: &[String], flag: &str) -> Result<String, String> {
    flag_value(args, flag).ok_or_else(|| format!("missing required {flag} <value>"))
}

fn flag_present(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn trailing_project_root(args: &[String]) -> Option<String> {
    const VALUE_FLAGS: &[&str] = &[
        "--language",
        "--language-id",
        "--owner",
        "--owner-path",
        "--read",
        "--op",
        "--locator",
        "--snippet",
        "--expected-snippet",
        "--mechanical-kind",
        "--max-edits",
        "--project-root",
        "--provider",
        "--provider-id",
        "--binary",
        "--namespace",
        "--item-name",
        "--item-kind",
        "--field",
        "--mutation-source",
        "--snippet-required",
        "--code-in-prompt",
    ];
    let values: Vec<_> = args
        .iter()
        .scan(false, |skip_next, arg| {
            if *skip_next {
                *skip_next = false;
                return Some(None);
            }
            if VALUE_FLAGS.contains(&arg.as_str()) {
                *skip_next = true;
                return Some(None);
            }
            Some((!arg.starts_with("--")).then(|| arg.clone()))
        })
        .flatten()
        .collect();
    match values.as_slice() {
        [project_root] => Some(project_root.clone()),
        _ => None,
    }
}

fn default_provider_for_language(language_id: &str) -> (Option<String>, Option<String>) {
    match language_id {
        "rust" => (
            Some("rs-harness".to_string()),
            Some("rs-harness".to_string()),
        ),
        "typescript" => (
            Some("ts-harness".to_string()),
            Some("ts-harness".to_string()),
        ),
        "python" => (
            Some("py-harness".to_string()),
            Some("py-harness".to_string()),
        ),
        _ => (None, None),
    }
}

fn location_from_read(read: &str) -> Option<(String, String)> {
    let mut parts = read.rsplitn(3, ':');
    let end = parts.next()?;
    let start = parts.next()?;
    let path = parts.next()?;
    Some((path.to_string(), format!("{start}:{end}")))
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
    let mutation_source = string_field(operation, "mutationSource");
    if let Some(source) = mutation_source.as_deref() {
        if matches!(
            source,
            "provider-native" | "agent-snippet" | "codex-text-fallback"
        ) {
            verification.push("mutation-source-valid");
        } else {
            failures.push(format!("unsupported mutationSource {source}"));
        }
    }
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
            if operation_name == "split_owner_items" {
                let fields = operation.get("fields").unwrap_or(&Value::Null);
                if string_field(fields, "destinationPath").is_some() {
                    verification.push("destination-path-present");
                } else {
                    failures.push(
                        "operation.fields.destinationPath is required for split_owner_items"
                            .to_string(),
                    );
                }
                if string_field(fields, "moduleName").is_some() {
                    verification.push("module-name-present");
                } else {
                    failures.push(
                        "operation.fields.moduleName is required for split_owner_items".to_string(),
                    );
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
                .map(|read| {
                    let provider_native = mutation_source.as_deref() == Some("provider-native");
                    MechanicalEditPlan {
                        kind: if provider_native {
                            "provider-native-dry-run"
                        } else {
                            "codex-dry-run"
                        },
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
                        requires_codex_apply_patch: !provider_native,
                        changed_ranges: vec![read.clone()],
                        notes: vec![if provider_native {
                            "provider-native AST mutation intent verified; use provider ast-patch apply after dry-run receipt".to_string()
                        } else if allow_large_mechanical_edit {
                            "bounded mechanical edit intent verified; provider AST dry-run should estimate exact affected nodes before mutation".to_string()
                        } else {
                            "single-target AST patch intent verified; Codex apply_patch remains only an explicit text fallback".to_string()
                        }],
                    }
                })
        })
    } else {
        None
    };
    let next = next_guidance_for_receipt(
        string_field(packet, "languageId").as_deref(),
        receipt_target.owner_path.as_deref(),
        receipt_target.read.as_deref(),
        op.as_deref(),
        mutation_source.as_deref(),
        status,
    );
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
        next,
    }
}

fn next_guidance_for_receipt(
    language_id: Option<&str>,
    owner_path: Option<&str>,
    read: Option<&str>,
    operation: Option<&str>,
    mutation_source: Option<&str>,
    status: &str,
) -> String {
    let language = language_id.unwrap_or("<language>");
    let owner = owner_path.unwrap_or("<owner-path>");
    let read = read.unwrap_or("<path:start:end>");
    let operation = operation.unwrap_or("<op>");
    if status == "failed" {
        return format!(
            "revise packet: asp ast-patch template --language {language} --owner {owner} --read {read} --op {operation} --field <key=value> [--snippet '<source when required>'] > semantic-ast-patch.json; rerun: asp {language} ast-patch dry-run --packet semantic-ast-patch.json ."
        );
    }
    if mutation_source == Some("provider-native") {
        return format!(
            "provider-dry-run: asp {language} ast-patch dry-run --packet semantic-ast-patch.json .; provider-apply: asp {language} ast-patch apply --packet semantic-ast-patch.json .; exact-read: asp {language} query --from-hook direct-source-read --selector {read} --code .; check: asp {language} check --changed ."
        );
    }
    format!(
        "provider-dry-run: asp {language} ast-patch dry-run --packet semantic-ast-patch.json .; exact-read: asp {language} query --from-hook direct-source-read --selector {read} --code .; fallback: Codex apply_patch only when mutationSource=codex-text-fallback or receipt.requiresCodexApplyPatch=true; check: asp {language} check --changed ."
    )
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
    "usage: asp ast-patch <verify|dry-run> --packet <path-or-> [PROJECT_ROOT]\n       asp ast-patch template --language <language> --owner <path> --read <path:start:end> --op <operation> [--snippet <source>] [--expected-snippet <source>] [PROJECT_ROOT]".to_string()
}
