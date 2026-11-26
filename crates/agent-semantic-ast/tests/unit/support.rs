// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Lightweight protocol test fixtures simulating external HTTP-JSON language
//! providers (`asp-braced-demo` and `asp-sexp-demo`) with zero embedded parsers.

use std::sync::Arc;

use agent_semantic_ast::{
    AST_INSPECT_OPERATION, AST_VERIFY_OPERATION, AstInspectRequest, AstInspectResponse,
    AstLanguageKind, AstNodeLocator, AstNodeSpanRecord, AstSyntaxDiagnostic, AstUniversalKind,
    AstVerifyCandidateResult, AstVerifyRequest, AstVerifyResponse, AstWorkspacePath, ByteSpan,
};
use agent_semantic_http_json::{HttpJsonRequest, HttpJsonResponse, serve_http_json};
use agent_semantic_provider_protocol::ProviderRegistrationDocument;
use agent_semantic_provider_transport::{
    ProviderRuntimeRequestFrame, ProviderRuntimeResponseFrame,
};
use tokio::net::TcpListener;
use tokio::sync::watch;

pub fn inspect_braced_source(path: &AstWorkspacePath, source: &str) -> AstInspectResponse {
    let mut nodes = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0usize;
    let mut depth = 0i32;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"//") {
            i = advance_to_newline(bytes, i);
            continue;
        }
        if depth == 0
            && let Some((node, next_i)) = try_parse_braced_item(path, source, i)
        {
            nodes.push(node);
            i = next_i;
            continue;
        }
        depth += match bytes[i] {
            b'{' => 1,
            b'}' => -1,
            _ => 0,
        };
        i += 1;
    }
    let diagnostics = verify_braced_source(source);
    AstInspectResponse {
        accepted: diagnostics.is_empty(),
        nodes,
        edges: Vec::new(),
        diagnostics,
        syntax_surface: None,
    }
}

fn try_parse_braced_item(
    path: &AstWorkspacePath,
    source: &str,
    start: usize,
) -> Option<(AstNodeSpanRecord, usize)> {
    let (prefix, kind, lang_kind, tag) = if source[start..].starts_with("struct ") {
        ("struct ", AstUniversalKind::Struct, "struct_decl", "struct")
    } else if source[start..].starts_with("fn ") {
        ("fn ", AstUniversalKind::Function, "fn_decl", "fn")
    } else {
        return None;
    };
    let name_start = start + prefix.len();
    let name_len = source.as_bytes()[name_start..]
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
        .count();
    let name = &source[name_start..name_start + name_len];
    let open = source[name_start + name_len..].find('{')? + name_start + name_len;
    let close = find_matching_delim(source, open, b'{', b'}', b"//")?;
    let line = 1 + source[..start].bytes().filter(|&b| b == b'\n').count();
    let span = ByteSpan {
        start,
        end: close + 1,
    };
    Some((
        AstNodeSpanRecord {
            locator: AstNodeLocator::new(format!("{}#{tag}:{name}", path.as_str()))
                .expect("valid locator"),
            kind,
            language_kind: AstLanguageKind::new(lang_kind).expect("lang kind"),
            name: name.to_owned(),
            line,
            outer_span: span,
            inner_span: span,
            body_span: Some(ByteSpan {
                start: open,
                end: close + 1,
            }),
            parent: None,
            attrs: Vec::new(),
        },
        close + 1,
    ))
}

pub fn verify_braced_source(source: &str) -> Vec<AstSyntaxDiagnostic> {
    if let Some(pos) = source.find("```") {
        return vec![AstSyntaxDiagnostic {
            byte_start: pos,
            byte_end: pos + 3,
            message: "unexpected markdown code fence".to_owned(),
            expected_tokens: Vec::new(),
        }];
    }
    let bytes = source.as_bytes();
    let mut stack = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"//") {
            i = advance_to_newline(bytes, i);
            continue;
        }
        if let Some(diag) = step_bracket_stack(bytes[i], i, &mut stack) {
            return vec![diag];
        }
        i += 1;
    }
    stack
        .last()
        .map(|&(exp, pos)| AstSyntaxDiagnostic {
            byte_start: pos,
            byte_end: source.len(),
            message: format!("unclosed delimiter, expected `{}`", exp as char),
            expected_tokens: vec![(exp as char).to_string()],
        })
        .into_iter()
        .collect()
}

pub fn inspect_sexp_source(path: &AstWorkspacePath, source: &str) -> AstInspectResponse {
    let mut nodes = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b';' {
            i = advance_to_newline(bytes, i);
            continue;
        }
        if bytes[i..].starts_with(b"(def (")
            && let Some((node, next_i)) = try_parse_sexp_def(path, source, i)
        {
            nodes.push(node);
            i = next_i;
            continue;
        }
        i += 1;
    }
    let diagnostics = verify_sexp_source(source);
    AstInspectResponse {
        accepted: diagnostics.is_empty(),
        nodes,
        edges: Vec::new(),
        diagnostics,
        syntax_surface: None,
    }
}

fn try_parse_sexp_def(
    path: &AstWorkspacePath,
    source: &str,
    start: usize,
) -> Option<(AstNodeSpanRecord, usize)> {
    let name_start = start + 6;
    let name_len = source.as_bytes()[name_start..]
        .iter()
        .take_while(|b| !b.is_ascii_whitespace() && **b != b')')
        .count();
    let name = &source[name_start..name_start + name_len];
    let close = find_matching_delim(source, start, b'(', b')', b";")?;
    let line = 1 + source[..start].bytes().filter(|&b| b == b'\n').count();
    let span = ByteSpan {
        start,
        end: close + 1,
    };
    Some((
        AstNodeSpanRecord {
            locator: AstNodeLocator::new(format!("{}#def:{name}", path.as_str()))
                .expect("valid locator"),
            kind: AstUniversalKind::Function,
            language_kind: AstLanguageKind::new("scheme_def").expect("lang kind"),
            name: name.to_owned(),
            line,
            outer_span: span,
            inner_span: span,
            body_span: Some(span),
            parent: None,
            attrs: Vec::new(),
        },
        close + 1,
    ))
}

fn verify_sexp_source(source: &str) -> Vec<AstSyntaxDiagnostic> {
    let bytes = source.as_bytes();
    let mut stack = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b';' {
            i = advance_to_newline(bytes, i);
            continue;
        }
        if let Some(diag) = step_bracket_stack(bytes[i], i, &mut stack) {
            return vec![diag];
        }
        i += 1;
    }
    stack
        .last()
        .map(|&(exp, pos)| AstSyntaxDiagnostic {
            byte_start: pos,
            byte_end: source.len(),
            message: format!("unclosed S-expression, expected `{}`", exp as char),
            expected_tokens: vec![(exp as char).to_string()],
        })
        .into_iter()
        .collect()
}

fn step_bracket_stack(
    byte: u8,
    pos: usize,
    stack: &mut Vec<(u8, usize)>,
) -> Option<AstSyntaxDiagnostic> {
    match byte {
        b'(' => stack.push((b')', pos)),
        b'{' => stack.push((b'}', pos)),
        b'[' => stack.push((b']', pos)),
        b')' | b'}' | b']' => {
            if stack.last().is_some_and(|(exp, _)| *exp == byte) {
                stack.pop();
            } else {
                return Some(AstSyntaxDiagnostic {
                    byte_start: pos,
                    byte_end: pos + 1,
                    message: format!("unexpected closer `{}`", byte as char),
                    expected_tokens: Vec::new(),
                });
            }
        }
        _ => {}
    }
    None
}

fn find_matching_delim(
    source: &str,
    open_idx: usize,
    open: u8,
    close: u8,
    comment_prefix: &[u8],
) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0i32;
    let mut i = open_idx;
    while i < bytes.len() {
        if bytes[i..].starts_with(comment_prefix) {
            i = advance_to_newline(bytes, i);
            continue;
        }
        if bytes[i] == open {
            depth += 1;
        } else if bytes[i] == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn advance_to_newline(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |off| start + off)
}

pub async fn spawn_http_provider_server<F>(
    dispatch: F,
) -> (String, watch::Sender<bool>, tokio::task::JoinHandle<()>)
where
    F: Fn(&str, serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback listener");
    let addr = listener.local_addr().expect("local addr");
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let dispatch = Arc::new(dispatch);
    let handle = tokio::spawn(async move {
        let _ = serve_http_json(listener, shutdown_rx, move |req: HttpJsonRequest| {
            let dispatch = Arc::clone(&dispatch);
            async move {
                let frame: ProviderRuntimeRequestFrame = serde_json::from_slice(&req.body)
                    .map_err(|err| format!("decode frame: {err}"))?;
                let response_frame = match dispatch(frame.operation.as_str(), frame.payload) {
                    Ok(value) => ProviderRuntimeResponseFrame::ready(frame.request_id, value),
                    Err(err) => ProviderRuntimeResponseFrame::error(frame.request_id, err),
                };
                let value = serde_json::to_value(response_frame)
                    .map_err(|err| format!("encode response frame: {err}"))?;
                HttpJsonResponse::json(200, &value)
            }
        })
        .await;
    });
    (format!("http://{addr}"), shutdown_tx, handle)
}

pub fn braced_http_dispatch(
    op: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    match op {
        AST_INSPECT_OPERATION => {
            let req: AstInspectRequest =
                serde_json::from_value(payload).map_err(|e| e.to_string())?;
            serde_json::to_value(inspect_braced_source(&req.path, &req.source_utf8))
                .map_err(|e| e.to_string())
        }
        AST_VERIFY_OPERATION => {
            let req: AstVerifyRequest =
                serde_json::from_value(payload).map_err(|e| e.to_string())?;
            let results = req
                .candidates
                .into_iter()
                .map(|cand| {
                    let diagnostics = verify_braced_source(&cand.source_utf8);
                    AstVerifyCandidateResult {
                        candidate_id: cand.candidate_id,
                        accepted: diagnostics.is_empty(),
                        diagnostics,
                    }
                })
                .collect();
            serde_json::to_value(AstVerifyResponse { results }).map_err(|e| e.to_string())
        }
        other => Err(format!("unsupported operation: {other}")),
    }
}

pub fn sexp_http_dispatch(
    op: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    match op {
        AST_INSPECT_OPERATION => {
            let req: AstInspectRequest =
                serde_json::from_value(payload).map_err(|e| e.to_string())?;
            serde_json::to_value(inspect_sexp_source(&req.path, &req.source_utf8))
                .map_err(|e| e.to_string())
        }
        AST_VERIFY_OPERATION => {
            let req: AstVerifyRequest =
                serde_json::from_value(payload).map_err(|e| e.to_string())?;
            let results = req
                .candidates
                .into_iter()
                .map(|cand| {
                    let diagnostics = verify_sexp_source(&cand.source_utf8);
                    AstVerifyCandidateResult {
                        candidate_id: cand.candidate_id,
                        accepted: diagnostics.is_empty(),
                        diagnostics,
                    }
                })
                .collect();
            serde_json::to_value(AstVerifyResponse { results }).map_err(|e| e.to_string())
        }
        other => Err(format!("unsupported operation: {other}")),
    }
}

pub fn canonical_provider_document(
    language_id: &str,
    provider_id: &str,
    extensions: &[&str],
) -> ProviderRegistrationDocument {
    ProviderRegistrationDocument {
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        registration: serde_json::json!({
            "languageId": language_id,
            "providerId": provider_id,
            "sourceInventory": {
                "packageRoots": [],
                "configFiles": [],
                "sourceExtensions": extensions,
                "projectResolution": null,
                "documentResolution": {
                    "extensions": extensions,
                    "supportsGitCandidates": true
                }
            }
        }),
    }
}
