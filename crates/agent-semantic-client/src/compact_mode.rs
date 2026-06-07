//! ASP-owned compact output mode contract.

use agent_semantic_client_core::{ClientMethod, ClientRequest};

/// Agent-facing output mode inferred from the public CLI flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompactOutputMode {
    /// Compact frontier/read-plan output. This mode must not inline source.
    Frontier,
    /// Pure byte-preserving code output requested with `--code`.
    Code,
    /// Structured source packet requested with `--json --view read-packet`.
    ReadPacket,
    /// Other structured JSON output.
    Json,
}

pub(crate) fn request_compact_output_mode(request: &ClientRequest) -> CompactOutputMode {
    if has_view(&request.forwarded_args, "read-packet")
        && has_flag(&request.forwarded_args, "--json")
    {
        return CompactOutputMode::ReadPacket;
    }
    if has_flag(&request.forwarded_args, "--code") {
        return CompactOutputMode::Code;
    }
    if has_flag(&request.forwarded_args, "--json") {
        return CompactOutputMode::Json;
    }
    CompactOutputMode::Frontier
}

pub(crate) fn validate_compact_provider_stdout(
    request: &ClientRequest,
    stdout: &[u8],
) -> Result<(), String> {
    if !matches!(request.method, ClientMethod::Search | ClientMethod::Query) {
        return Ok(());
    }
    if request_compact_output_mode(request) != CompactOutputMode::Frontier {
        return Ok(());
    }
    let output = String::from_utf8_lossy(stdout);
    for (line_index, line) in output.lines().enumerate() {
        if line.starts_with("|code ") {
            return Err(inline_code_error(line_index + 1, "|code"));
        }
        if line.starts_with('|') && line.contains(" text=") {
            return Err(inline_code_error(line_index + 1, "text"));
        }
    }
    Ok(())
}

fn inline_code_error(line_number: usize, field: &str) -> String {
    format!(
        "provider violated ASP compact frontier mode at stdout line {line_number}: `{field}` inline source is forbidden; use --code for pure code or --json --view read-packet for structured source"
    )
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn has_view(args: &[String], expected: &str) -> bool {
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--view" => {
                if args
                    .get(index + 1)
                    .is_some_and(|value| value.as_str() == expected)
                {
                    return true;
                }
                index += 2;
            }
            arg if arg.strip_prefix("--view=") == Some(expected) => return true,
            _ => index += 1,
        }
    }
    false
}
