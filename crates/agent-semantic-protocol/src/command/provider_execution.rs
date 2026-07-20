use std::path::PathBuf;

use super::graph::GraphTurboReceiptRequest;

const FRONTIER_RECEIPT_FACT_FLAGS: &[(&str, &str)] = &[
    ("--frontier-receipt-follow-node", "--follow-node"),
    ("--frontier-receipt-read-selector", "--read-selector"),
    ("--frontier-receipt-read-kind", "--read-kind"),
    ("--frontier-receipt-read-owner", "--read-owner"),
    ("--frontier-receipt-test-argv-json", "--test-argv-json"),
    ("--frontier-receipt-test-status", "--test-status"),
    ("--frontier-receipt-test-summary", "--test-summary"),
    ("--frontier-receipt-test-exit-code", "--test-exit-code"),
    ("--frontier-receipt-test-workdir", "--test-workdir"),
    ("--frontier-receipt-test-fingerprint", "--test-fingerprint"),
    (
        "--frontier-receipt-commands-to-first-useful-locator",
        "--commands-to-first-useful-locator",
    ),
    (
        "--frontier-receipt-commands-to-validation",
        "--commands-to-validation",
    ),
];

pub(super) fn take_frontier_receipt_request(
    args: &mut Vec<String>,
) -> Result<Option<GraphTurboReceiptRequest>, String> {
    let mut normalized = Vec::with_capacity(args.len());
    let mut frontier_receipt_out = None;
    let mut receipt_args = Vec::new();
    let mut seen_fact_flags = Vec::<&'static str>::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--frontier-receipt-out" {
            if frontier_receipt_out.is_some() {
                return Err("--frontier-receipt-out may be passed only once".to_string());
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| "--frontier-receipt-out requires a path".to_string())?;
            frontier_receipt_out = Some(PathBuf::from(value));
            index += 2;
        } else if let Some(value) = arg.strip_prefix("--frontier-receipt-out=") {
            if frontier_receipt_out.is_some() {
                return Err("--frontier-receipt-out may be passed only once".to_string());
            }
            if value.is_empty() {
                return Err("--frontier-receipt-out requires a path".to_string());
            }
            frontier_receipt_out = Some(PathBuf::from(value));
            index += 1;
        } else if let Some((target_flag, value, public_flag)) =
            frontier_receipt_fact_arg(arg, args.get(index + 1).map(String::as_str))
        {
            if seen_fact_flags.contains(&public_flag) {
                return Err(format!("{public_flag} may be passed only once"));
            }
            if value.is_empty() {
                if public_flag == "--view" && args.iter().any(|arg| arg == "lexical") {
                    return Err("search lexical --view requires seeds".to_string());
                }
                return Err(format!("{public_flag} requires a value"));
            }
            seen_fact_flags.push(public_flag);
            receipt_args.push(target_flag.to_string());
            receipt_args.push(value.to_string());
            if arg == public_flag {
                index += 2;
            } else {
                index += 1;
            }
        } else {
            normalized.push(arg.clone());
            index += 1;
        }
    }
    *args = normalized;
    let Some(out_path) = frontier_receipt_out else {
        if receipt_args.is_empty() {
            return Ok(None);
        }
        return Err("--frontier-receipt-* fact flags require --frontier-receipt-out".to_string());
    };
    Ok(Some(GraphTurboReceiptRequest::new(out_path, receipt_args)))
}

fn frontier_receipt_fact_arg<'a>(
    arg: &'a str,
    next: Option<&'a str>,
) -> Option<(&'static str, &'a str, &'static str)> {
    for (public_flag, target_flag) in FRONTIER_RECEIPT_FACT_FLAGS {
        if arg == *public_flag {
            return Some((*target_flag, next.unwrap_or(""), *public_flag));
        }
        let prefix = format!("{public_flag}=");
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some((*target_flag, value, *public_flag));
        }
    }
    None
}
pub(super) fn provider_process_args(args: &[String]) -> Vec<String> {
    args.to_vec()
}
