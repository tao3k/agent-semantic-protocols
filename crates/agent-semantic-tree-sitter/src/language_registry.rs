//! ASP-owned registry for concrete `tree-sitter` language grammars.

use tree_sitter::Language;

struct RegisteredLanguageGrammar {
    language_id: &'static str,
    load: fn() -> Language,
}

const REGISTERED_LANGUAGE_GRAMMARS: &[RegisteredLanguageGrammar] = &[RegisteredLanguageGrammar {
    language_id: "rust",
    load: rust_language,
}];

/// Resolve one grammar from the ASP-owned language registry.
pub fn registered_language_grammar(language_id: &str) -> Result<Language, String> {
    REGISTERED_LANGUAGE_GRAMMARS
        .iter()
        .find(|grammar| grammar.language_id == language_id)
        .map(|grammar| (grammar.load)())
        .ok_or_else(|| {
            format!("tree-sitter workspace queries are not registered for language `{language_id}`")
        })
}

fn rust_language() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}
