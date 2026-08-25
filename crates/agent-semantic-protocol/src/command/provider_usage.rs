use super::provider_selector::registered_language_facades_line;

pub(super) fn is_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "guide")
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
