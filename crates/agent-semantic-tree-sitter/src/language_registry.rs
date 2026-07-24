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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredTreeSitterLanguageId<'a>(&'a str);

impl<'a> RegisteredTreeSitterLanguageId<'a> {
    pub fn as_str(&self) -> &str {
        self.0
    }
}

impl<'a> From<&'a str> for RegisteredTreeSitterLanguageId<'a> {
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

/// Resolve one grammar from the ASP-owned language registry.
pub fn registered_language_grammar(
    language_id: RegisteredTreeSitterLanguageId<'_>,
) -> Result<Language, String> {
    REGISTERED_LANGUAGE_GRAMMARS
        .iter()
        .find(|grammar| grammar.language_id == language_id.as_str())
        .map(|grammar| (grammar.load)())
        .ok_or_else(|| {
            format!(
                "tree-sitter workspace queries are not registered for language `{}`",
                language_id.as_str()
            )
        })
}

fn rust_language() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}
