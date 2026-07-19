use super::SUPPORTED_COMMANDS;
use super::provider_facades::registered_language_facades_line;

pub(super) fn validate_provider_command(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(provider_usage());
    };
    let supported = if command == "agent" {
        args.get(1)
            .is_some_and(|subcommand| matches!(subcommand.as_str(), "doctor"))
    } else {
        SUPPORTED_COMMANDS.contains(&command)
    };
    if supported {
        Ok(())
    } else {
        Err(provider_usage())
    }
}

fn is_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "guide")
}

pub(super) fn provider_guide_args(language_id: &str, args: &[String]) -> Vec<String> {
    if matches!(language_id, "python" | "typescript") && is_guide(args) {
        let mut rewritten = vec!["agent".to_string(), "guide".to_string()];
        rewritten.extend(args.iter().skip(1).cloned());
        rewritten
    } else {
        args.to_vec()
    }
}

pub(super) fn provider_usage() -> String {
    format!(
        "usage: asp <{}> [--help|--version] <guide|search|query|check|cache|info|bench|projection|agent doctor|ast-patch|evidence> ...\nprojection: import --owner <relative-owner-path> --workspace <root>\nsearch: pipe|lexical|deps|dependency|ingest|failure|reasoning|owner|guide|prime\nsearch deps: current manifest dependency topology and dependency-owned next actions",
        registered_language_facades_line()
    )
}

pub(super) fn guide_usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} guide [--help] [--workspace <root>]\n\nPrints the low-frequency provider-owned agent tool map.\nUse `asp {language_id} search guide --workspace .`, `asp {language_id} query guide --workspace .`, or `asp {language_id} query guide treesitter --workspace .` for focused reference guides."
    )
}
