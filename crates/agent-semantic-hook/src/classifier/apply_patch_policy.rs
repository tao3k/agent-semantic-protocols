//! Owns semantic AST patch admission for registered-language source paths.

use serde_json::Value;

use super::decision::{DenyForActionRequest, allow, deny_for_action};
use super::{direct_read_language_ids, policy_direct_read_routes};
use crate::command::{apply_patch_source_paths, command_line};
use crate::{
    HookDecision, HookRuntime, OperationIntent, ReasonKind, ToolAction,
    collect_source_selector_matches, subject_for_action,
};

pub(super) fn materialize_apply_patch_decision(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    semantic_ast_patch_enabled: bool,
) -> Option<HookDecision> {
    if !semantic_ast_patch_enabled {
        return None;
    }
    if action.operation == OperationIntent::ApplyPatch && !action.paths.is_empty() {
        return classify_apply_patch_paths(registry, platform, event, action, action.paths.clone());
    }
    let command = action.command.as_deref()?;
    let patch_paths = apply_patch_source_paths(&action.tool_name, command);
    if patch_paths.is_empty() {
        return None;
    }
    classify_apply_patch_paths(registry, platform, event, action, patch_paths)
}

pub(super) fn classify_apply_patch_paths(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    patch_paths: Vec<String>,
) -> Option<HookDecision> {
    let matches =
        collect_source_selector_matches(registry, patch_paths.iter().map(String::as_str), |_| true);
    if matches.is_empty() {
        return None;
    }
    let routes = policy_direct_read_routes(&matches);
    let mut subject = subject_for_action(action);
    subject.paths = patch_paths;
    let languages = direct_read_language_ids(&matches);
    if let Some(command) = action.command.as_deref() {
        let patch_digest = source_apply_patch_digest(command);
        let authorization_path = source_apply_patch_authorization_path(registry, &patch_digest);
        if authorization_path.is_file() {
            let mut decision = allow(platform, event, subject);
            decision.language_ids = languages;
            decision.message = format!(
                "source apply_patch allowed by controlled maintenance authorization {}",
                authorization_path.display()
            );
            decision.fields.insert(
                "toolSurface".to_string(),
                Value::String(action.surface.as_str().to_string()),
            );
            decision.fields.insert(
                "operationIntent".to_string(),
                Value::String(action.operation.as_str().to_string()),
            );
            decision.fields.insert(
                "maintenancePolicy".to_string(),
                Value::String("source-apply-patch-authorization".to_string()),
            );
            decision
                .fields
                .insert("patchDigest".to_string(), Value::String(patch_digest));
            decision.fields.insert(
                "authorizationPath".to_string(),
                Value::String(authorization_path.display().to_string()),
            );
            return Some(decision);
        }
    }
    let language = languages
        .first()
        .map(agent_semantic_config::LanguageId::as_str)
        .unwrap_or("<language>");
    let project_root = routes
        .first()
        .and_then(|route| route.argv.last())
        .filter(|arg| !arg.starts_with('-'))
        .map(String::as_str)
        .unwrap_or(".");
    let route_guide = routes
        .iter()
        .map(|route| command_line(&route.argv))
        .collect::<Vec<_>>()
        .join("; ");
    let message = format!(
        "source apply_patch denied; handwritten source hunks are not a supported workflow for protected source. Locator route: {route_guide}. Treat path-only locator output as a frontier, not patch preimage; exact patch context must come from `asp {language} query --selector <exact-structural-selector> --workspace {project_root} --projection source`. Build semantic-ast-patch.json with `asp ast-patch template --language {language} --owner <owner-path> --read <exact-structural-selector> --op <operation> --field <key=value> {project_root}`; verify with `asp {language} ast-patch dry-run --packet semantic-ast-patch.json {project_root}`; apply with provider-native `asp {language} ast-patch apply --packet semantic-ast-patch.json {project_root}` when the receipt reports mutationSource=provider-native. Codex text patching is only a codex-text-fallback or controlled maintenance policy path, not the normal AST patch route."
    );
    Some(deny_for_action(
        platform,
        event,
        DenyForActionRequest {
            reason_kind: ReasonKind::SemanticAstPatchRequired,
            action,
            language_ids: languages,
            subject,
            routes,
            message,
        },
    ))
}

fn source_apply_patch_authorization_path(
    registry: &HookRuntime,
    patch_digest: &str,
) -> std::path::PathBuf {
    std::path::Path::new(&registry.project_root)
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("source-apply-patch")
        .join(format!("{patch_digest}.json"))
}

fn source_apply_patch_digest(command: &str) -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(command.as_bytes());
    format!("{digest:x}")
}
