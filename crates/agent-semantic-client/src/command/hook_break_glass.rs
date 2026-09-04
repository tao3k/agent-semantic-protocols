//! Operator-approved one-shot Hook break-glass capability minting.

use agent_semantic_hook::latest_hook_session_agent_route;
use clap::Arg;
use clap::Command;
use clap::builder::PossibleValuesParser;
use clap::value_parser;
use std::path::Path;
use std::path::PathBuf;

const DEFECT_KINDS: &[&str] = &[
    "admitted-row-state-mismatch",
    "missing-required-disclosure",
    "invalid-history-cursor",
    "exhausted-non-progress-cycle",
    "runtime-workspace-unadmitted",
    "provider-bootstrap-deadlock",
];

pub(super) fn run_hook_break_glass(args: &[String]) -> Result<(), String> {
    let mut argv = vec!["asp hook break-glass".to_owned()];
    argv.extend(args.iter().cloned());
    let matches = break_glass_command()
        .try_get_matches_from(argv)
        .map_err(|error| error.to_string())?;
    let Some(("mint", mint)) = matches.subcommand() else {
        return Err(break_glass_command().render_usage().to_string());
    };
    let project_root = mint
        .get_one::<PathBuf>("project-root")
        .map(PathBuf::as_path)
        .unwrap_or_else(|| Path::new("."));
    let protected_command = mint
        .get_one::<String>("command")
        .ok_or_else(|| "break-glass mint requires --command".to_owned())?;
    let defect_kind = mint
        .get_one::<String>("defect-kind")
        .ok_or_else(|| "break-glass mint requires --defect-kind".to_owned())?;
    let ttl_seconds = *mint
        .get_one::<u64>("ttl-seconds")
        .ok_or_else(|| "break-glass mint requires --ttl-seconds".to_owned())?;
    let route = latest_hook_session_agent_route(project_root)?
        .ok_or_else(|| "break-glass mint requires a current denied Hook route".to_owned())?;
    if route.subject_command.as_deref() != Some(protected_command.as_str()) {
        return Err(
            "break-glass protected command must exactly match the newest denied Hook command"
                .to_owned(),
        );
    }
    let deny_evidence_ref = route
        .deny_evidence_ref
        .as_deref()
        .ok_or_else(|| "break-glass mint requires a typed deny evidence reference".to_owned())?;
    let capability = crate::hook_break_glass::issue_hook_break_glass_capability(
        crate::hook_break_glass::HookBreakGlassIssue {
            workspace_root: project_root,
            root_session_id: &route.root_session_id,
            protected_command,
            deny_evidence_ref,
            defect_kind,
            ttl_seconds,
        },
    )?;
    println!(
        "{}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.hook-break-glass-mint-receipt",
            "schemaVersion": "1",
            "state": "pending",
            "nonce": capability.nonce,
            "workspaceRoot": capability.workspace_root,
            "rootSessionId": capability.root_session_id,
            "protectedCommandDigest": capability.protected_command_digest,
            "denyEvidenceRef": capability.deny_evidence_ref,
            "defectKind": capability.defect_kind,
            "expiresAtUnixMs": capability.expires_at_unix_ms,
            "oneShotCommand": format!(
                "ASP_BREAK_GLASS_CAPABILITY={} {}",
                capability.nonce, protected_command,
            ),
        })
    );
    Ok(())
}

pub(super) fn break_glass_command() -> Command {
    Command::new("asp hook break-glass")
        .about("Mint an operator-approved, one-shot Hook defect capability")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("mint")
                .about("Bind one denied command to a short-lived one-shot capability")
                .arg(
                    Arg::new("defect-kind")
                        .long("defect-kind")
                        .required(true)
                        .value_parser(PossibleValuesParser::new(DEFECT_KINDS)),
                )
                .arg(Arg::new("command").long("command").required(true))
                .arg(
                    Arg::new("ttl-seconds")
                        .long("ttl-seconds")
                        .default_value("30")
                        .value_parser(value_parser!(u64).range(1..=60)),
                )
                .arg(
                    Arg::new("project-root")
                        .value_name("PROJECT_ROOT")
                        .value_parser(value_parser!(PathBuf))
                        .default_value("."),
                ),
        )
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_break_glass.rs"]
mod tests;
