use super::provider_selector::registered_language_facades_line;

pub(super) fn is_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "guide")
}

pub(super) fn provider_usage() -> String {
    format!(
        "usage: asp <{}> [--help|--version] <guide|search|query|check|cache|info|bench|projection|agent doctor|ast-patch|evidence> ...\nprojection: import --owner <relative-owner-path> --workspace <root>\nsearch: playbook <query> [--intent conceptual|relationship|exact-literal|absence-proof] [--scope workspace|owner:<path>] [--coverage candidates|complete] [--max-owners <1..100>] [--deadline-ms <1..5000>] [--explain compact|full]\nquery: --selector <parser-owned-selector> [--projection source]",
        registered_language_facades_line()
    )
}

pub(super) fn guide_usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} guide [--help] [--workspace <root>]\n\nPrints the low-frequency provider-owned agent tool map.\nUse `asp {language_id} search guide --workspace .`, `asp {language_id} query guide --workspace .`, or `asp {language_id} query guide treesitter --workspace .` for focused reference guides."
    )
}
