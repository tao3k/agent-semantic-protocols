use super::provider_selector::registered_language_facades_line;

pub(super) fn is_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "guide")
}

pub(super) fn provider_usage() -> String {
    format!(
        "usage: asp <{}> [--help|--version] <guide|check|cache|info|bench|projection|agent doctor|ast-patch|evidence> ...\nprojection: import --owner <relative-owner-path> --workspace <root>\nSearch and Query are root operations: `asp search playbook --languages <language|...> ...` and `asp query --selector <parser-owned-selector>`.",
        registered_language_facades_line()
    )
}

pub(super) fn guide_usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} guide [--help] [--workspace <root>]\n\nPrints the low-frequency provider-owned agent tool map. Search Playbook recovers the provider-owned Example and Grammar through `asp search playbook --languages {language_id}`."
    )
}
