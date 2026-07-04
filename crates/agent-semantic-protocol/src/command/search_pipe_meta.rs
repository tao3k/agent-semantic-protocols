//! Help and version handling for ASP-owned fast search commands.

use super::protocol_version_line;

pub(super) fn run_asp_fast_search_meta_command(language_id: &str, args: &[String]) -> bool {
    if !matches!(args.first().map(String::as_str), Some("search")) {
        return false;
    }
    if args.iter().skip(1).any(|arg| is_help_arg(arg)) {
        println!(
            "{}",
            fast_search_usage(language_id, args.get(1).map(String::as_str))
        );
        return true;
    }
    if args.iter().skip(1).any(|arg| is_version_arg(arg)) {
        println!("{}", protocol_version_line());
        return true;
    }
    false
}

fn is_help_arg(arg: &str) -> bool {
    matches!(arg, "--help" | "-h")
}

fn is_version_arg(arg: &str) -> bool {
    matches!(arg, "--version" | "-V")
}

fn fast_search_usage(language_id: &str, subcommand: Option<&str>) -> String {
    match subcommand {
        Some("pipe") => format!(
            "usage: asp {language_id} search pipe <refinement-query> [--selector SELECTOR] [--query TERMS] [--workspace PROJECT_ROOT] [--source auto|provider|search-overlay|ingest] [--view seeds|graph-turbo-request] [scope...]\n\nBuilds an ASP-owned refinement frontier after lexical/dependency evidence is ambiguous. Use --selector with --query to bind an exact code owner and context terms without shell-joining query/search commands. Do not use pipe for CLI-command lexical searches."
        ),
        Some("lexical") => format!(
            "usage: asp {language_id} search lexical <term-or-error> [items|tests|deps] [--view seeds|graph-turbo-request] [owner...]\n\nRuns bounded lexical recall through the dynamic overlay/source index and renders an ASP-owned search frontier."
        ),
        Some("deps" | "dependency") => format!(
            "usage: asp {language_id} search deps <dependency-or-api> [api-term] [--workspace PROJECT_ROOT] [--view hits|seeds|public-external-types]\n\nReads current manifest dependency topology and renders dependency-owned next actions."
        ),
        Some("ingest") => format!(
            "usage: asp {language_id} search ingest [items|tests|deps] [--view seeds|graph-turbo-request]\n\nReads candidate lines from stdin and renders an ASP-owned search frontier."
        ),
        Some("failure") => format!(
            "usage: asp {language_id} search failure (--message <text>|--from-last-check|<failure text>) --view seeds|graph-turbo-request\n\nProjects a failure transcript into hot selectors and next actions."
        ),
        Some("reasoning") => format!(
            "usage: asp {language_id} search reasoning <owner-query|owner-tests> [OPTIONS] --view seeds\n\nRuns a focused graph reasoning search profile."
        ),
        Some("owner") => format!(
            "usage: asp {language_id} search owner <owner-path> items --query <symbol-or-a|b|c> --view seeds\n\nRanks owner-local items for an LLM-generated symbol/API query-set."
        ),
        _ => format!(
            "usage: asp {language_id} search <pipe|lexical|deps|dependency|ingest|failure|reasoning|owner|guide|prime> ...\n\nUse --help after a search subcommand for focused usage.\nsearch deps: current manifest dependency topology and dependency-owned next actions."
        ),
    }
}
