//! Codex-internal source access policy command.

use agent_semantic_hook::load_activation;
use agent_semantic_hook::source_access::{
    SourceAccessDecision, codex_fs_read_file_decision, codex_shell_egress_suppression_decision,
};
use std::path::PathBuf;

pub(crate) fn run_source_access_command(args: &[String]) -> Result<(), String> {
    let decision = source_access_decision_from_args(args)?;
    let text = serde_json::to_string_pretty(&decision)
        .map_err(|error| format!("failed to serialize source-access decision: {error}"))?;
    println!("{text}");
    Ok(())
}

pub(crate) fn source_access_decision_from_args(
    args: &[String],
) -> Result<Option<SourceAccessDecision>, String> {
    let Some(kind) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    let parsed = SourceAccessArgs::parse(&args[1..])?;
    let activation_path = parsed.activation.clone().ok_or_else(|| {
        "source-access requires --activation <activation.json>; this command does not discover hook state".to_string()
    })?;
    let registry = load_activation(&activation_path)?;
    match kind {
        "read-file" => {
            let path = parsed.single_path(kind)?.to_string();
            Ok(codex_fs_read_file_decision(
                &registry,
                parsed
                    .rpc_method
                    .unwrap_or_else(|| "fs/readFile".to_string()),
                path,
            ))
        }
        "shell-egress" => {
            let path = parsed.single_path(kind)?.to_string();
            let command = parsed.command.ok_or_else(|| {
                "source-access shell-egress requires --command <command>".to_string()
            })?;
            let output_digest = parsed.output_digest.ok_or_else(|| {
                "source-access shell-egress requires --output-digest <digest>".to_string()
            })?;
            Ok(codex_shell_egress_suppression_decision(
                &registry,
                command,
                path,
                output_digest,
            ))
        }
        "help" | "--help" | "-h" => Err(usage()),
        other => Err(format!(
            "unknown source-access command `{other}`\n{}",
            usage()
        )),
    }
}

#[derive(Default)]
struct SourceAccessArgs {
    activation: Option<PathBuf>,
    rpc_method: Option<String>,
    command: Option<String>,
    output_digest: Option<String>,
    paths: Vec<String>,
}

impl SourceAccessArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self::default();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--activation" => {
                    index += 1;
                    parsed.activation =
                        Some(PathBuf::from(args.get(index).ok_or_else(|| {
                            "--activation requires a value".to_string()
                        })?));
                }
                "--rpc-method" => {
                    index += 1;
                    parsed.rpc_method = Some(
                        args.get(index)
                            .ok_or_else(|| "--rpc-method requires a value".to_string())?
                            .to_string(),
                    );
                }
                "--command" => {
                    index += 1;
                    parsed.command = Some(
                        args.get(index)
                            .ok_or_else(|| "--command requires a value".to_string())?
                            .to_string(),
                    );
                }
                "--output-digest" => {
                    index += 1;
                    parsed.output_digest = Some(
                        args.get(index)
                            .ok_or_else(|| "--output-digest requires a value".to_string())?
                            .to_string(),
                    );
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown source-access flag `{value}`\n{}", usage()));
                }
                value => parsed.paths.push(value.to_string()),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn single_path(&self, kind: &str) -> Result<&str, String> {
        match self.paths.as_slice() {
            [path] => Ok(path),
            [] => Err(format!("source-access {kind} requires a path")),
            _ => Err(format!("source-access {kind} accepts exactly one path")),
        }
    }
}

fn usage() -> String {
    "usage: asp source-access <read-file|shell-egress> --activation <activation.json> [FLAGS] <path>"
        .to_string()
}
