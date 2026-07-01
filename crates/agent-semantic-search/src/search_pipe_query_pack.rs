#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPipeTermRole {
    Context,
    Concept,
    Symbol,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeQueryTerm {
    pub raw: String,
    pub lower: String,
    pub role: SearchPipeTermRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeQueryClause {
    pub terms: Vec<SearchPipeQueryTerm>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeClauseCoverage {
    pub id: usize,
    pub matched: Vec<String>,
    pub missing: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeQueryPackCandidate {
    pub path: String,
    pub symbol: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueryTokenFragment {
    raw: String,
    force_symbol: bool,
}

impl SearchPipeTermRole {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Concept => "concept",
            Self::Symbol => "symbol",
        }
    }
}

#[must_use]
pub fn search_pipe_query_clauses(language_id: &str, query: &str) -> Vec<SearchPipeQueryClause> {
    let explicit = query
        .split('|')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .map(|raw_clause| SearchPipeQueryClause {
            terms: search_pipe_query_terms(language_id, raw_clause),
        })
        .filter(|clause| !clause.terms.is_empty())
        .collect::<Vec<_>>();
    if query.contains('|') {
        return explicit;
    }
    auto_query_clauses(explicit)
}

#[must_use]
pub fn search_pipe_query_clause_texts(language_id: &str, query: &str) -> Vec<String> {
    search_pipe_query_clauses(language_id, query)
        .into_iter()
        .map(|clause| {
            clause
                .terms
                .into_iter()
                .map(|term| term.raw)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|clause| !clause.is_empty())
        .collect()
}

#[must_use]
pub fn search_pipe_unique_query_terms(
    clauses: &[SearchPipeQueryClause],
) -> Vec<SearchPipeQueryTerm> {
    clauses
        .iter()
        .flat_map(|clause| clause.terms.iter())
        .fold(Vec::new(), |mut terms, term| {
            if !terms
                .iter()
                .any(|seen: &SearchPipeQueryTerm| seen.raw == term.raw)
            {
                terms.push(term.clone());
            }
            terms
        })
}

#[must_use]
pub fn search_pipe_clause_coverages(
    clauses: &[SearchPipeQueryClause],
    candidates: &[SearchPipeQueryPackCandidate],
) -> Vec<SearchPipeClauseCoverage> {
    clauses
        .iter()
        .enumerate()
        .map(|(index, clause)| {
            let matched = clause
                .terms
                .iter()
                .filter(|term| {
                    candidates
                        .iter()
                        .any(|candidate| search_pipe_query_candidate_matches_term(candidate, term))
                })
                .map(|term| term.lower.clone())
                .collect::<Vec<_>>();
            let missing = clause
                .terms
                .iter()
                .filter(|term| !matched.iter().any(|matched| matched == &term.lower))
                .map(|term| term.lower.clone())
                .collect::<Vec<_>>();
            SearchPipeClauseCoverage {
                id: index + 1,
                matched,
                missing,
            }
        })
        .collect()
}

#[must_use]
pub fn search_pipe_role_terms(
    terms: &[SearchPipeQueryTerm],
    role: SearchPipeTermRole,
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| term.role == role)
        .map(|term| term.raw.clone())
        .collect()
}

#[must_use]
pub fn search_pipe_next_query_pack_hint(
    context_terms: &[String],
    owner_seed_terms: &[String],
    concept_terms: &[String],
) -> Option<String> {
    if owner_seed_terms.len() < 2 {
        return None;
    }
    let mut clauses = vec![owner_seed_terms.join(" ")];
    if concept_terms
        .iter()
        .any(|term| term.eq_ignore_ascii_case("concurrency"))
    {
        clauses.push("concurrency runtime scheduling".to_string());
    } else if !concept_terms.is_empty() {
        clauses.push(concept_terms.join(" "));
    }
    if owner_seed_terms
        .iter()
        .any(|term| term.eq_ignore_ascii_case("Scope"))
    {
        clauses.push("Scope lifecycle".to_string());
    }
    if owner_seed_terms
        .iter()
        .any(|term| term.eq_ignore_ascii_case("Queue"))
        && owner_seed_terms
            .iter()
            .any(|term| term.eq_ignore_ascii_case("Stream"))
    {
        clauses.push("Queue Stream backpressure".to_string());
    }
    if clauses.len() == 1 && !context_terms.is_empty() {
        clauses.push(context_terms.join(" "));
    }
    Some(clauses.join("|"))
}

#[must_use]
pub fn search_pipe_is_path_like_token(raw: &str) -> bool {
    raw.contains('/') || raw.contains("::") || raw.contains('.') || raw.contains('_')
}

#[must_use]
pub fn search_pipe_query_candidate_matches_term(
    candidate: &SearchPipeQueryPackCandidate,
    term: &SearchPipeQueryTerm,
) -> bool {
    candidate.symbol.to_ascii_lowercase().contains(&term.lower)
        || candidate.path.to_ascii_lowercase().contains(&term.lower)
        || candidate.text.to_ascii_lowercase().contains(&term.lower)
}

fn term_role(language_id: &str, raw: &str) -> SearchPipeTermRole {
    if language_id == "typescript" && matches!(raw, "Effect") {
        return SearchPipeTermRole::Context;
    }
    if is_weak_natural_term(raw) {
        return SearchPipeTermRole::Context;
    }
    if is_owner_seed_token(raw) {
        return SearchPipeTermRole::Symbol;
    }
    if raw
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
    {
        return SearchPipeTermRole::Symbol;
    }
    SearchPipeTermRole::Concept
}

fn search_pipe_query_terms(language_id: &str, raw_clause: &str) -> Vec<SearchPipeQueryTerm> {
    raw_clause
        .split(|character: char| character == ',' || character.is_whitespace())
        .flat_map(query_token_fragments)
        .map(|fragment| SearchPipeQueryTerm {
            raw: fragment.raw.clone(),
            lower: fragment.raw.to_ascii_lowercase(),
            role: if fragment.force_symbol {
                SearchPipeTermRole::Symbol
            } else {
                term_role(language_id, &fragment.raw)
            },
        })
        .fold(Vec::new(), |mut terms, term| {
            if !terms
                .iter()
                .any(|seen: &SearchPipeQueryTerm| seen.raw == term.raw)
            {
                terms.push(term);
            }
            terms
        })
}

fn query_token_fragments(raw: &str) -> Vec<QueryTokenFragment> {
    let trimmed = trim_query_token(raw);
    if trimmed.is_empty() || !has_ascii_query_signal(trimmed) {
        return Vec::new();
    }
    if should_split_slash_compound(trimmed) {
        return trimmed
            .split('/')
            .flat_map(query_token_fragments)
            .collect::<Vec<_>>();
    }
    vec![QueryTokenFragment {
        raw: trimmed.to_string(),
        force_symbol: false,
    }]
}

fn trim_query_token(raw: &str) -> &str {
    raw.trim_matches(|character: char| !is_query_token_character(character))
}

fn is_query_token_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '/' | ':' | '.' | '@')
}

fn has_ascii_query_signal(raw: &str) -> bool {
    raw.chars()
        .any(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn should_split_slash_compound(raw: &str) -> bool {
    if raw.contains('.') || raw.contains("::") {
        return false;
    }
    let parts = raw.split('/').collect::<Vec<_>>();
    if parts.len() != 2 || parts.iter().any(|part| part.is_empty()) {
        return false;
    }
    if matches!(
        parts[0],
        "src"
            | "test"
            | "tests"
            | "crates"
            | "packages"
            | "apps"
            | "lib"
            | "libs"
            | "docs"
            | "examples"
            | "benches"
    ) {
        return false;
    }
    parts.iter().all(|part| {
        part.chars().all(|character| {
            character == '_' || character == '-' || character.is_ascii_alphanumeric()
        })
    })
}

fn auto_query_clauses(explicit: Vec<SearchPipeQueryClause>) -> Vec<SearchPipeQueryClause> {
    let Some(single) = explicit.first() else {
        return explicit;
    };
    if explicit.len() != 1 || single.terms.len() < 6 {
        return explicit;
    }

    let mut path_terms = Vec::new();
    let mut package_terms = Vec::new();
    let mut symbol_terms = Vec::new();
    let mut concept_terms = Vec::new();
    let mut context_terms = Vec::new();
    for term in &single.terms {
        if search_pipe_is_path_like_token(&term.raw) {
            path_terms.push(term.clone());
        } else if is_package_like_token(&term.raw) {
            package_terms.push(term.clone());
        } else {
            match term.role {
                SearchPipeTermRole::Symbol => symbol_terms.push(term.clone()),
                SearchPipeTermRole::Concept => concept_terms.push(term.clone()),
                SearchPipeTermRole::Context => context_terms.push(term.clone()),
            }
        }
    }

    let mut clauses = [path_terms, package_terms, symbol_terms, concept_terms]
        .into_iter()
        .filter(|terms| !terms.is_empty())
        .map(|terms| SearchPipeQueryClause { terms })
        .collect::<Vec<_>>();
    if clauses.is_empty() && !context_terms.is_empty() {
        clauses.push(SearchPipeQueryClause {
            terms: context_terms,
        });
    }
    if clauses.len() > 1 { clauses } else { explicit }
}

fn is_owner_seed_token(raw: &str) -> bool {
    search_pipe_is_path_like_token(raw) || is_package_like_token(raw)
}

fn is_package_like_token(raw: &str) -> bool {
    raw.matches('-').count() >= 2 && !matches!(raw, "long-field-signatures")
}

fn is_weak_natural_term(raw: &str) -> bool {
    matches!(
        raw.to_ascii_lowercase().as_str(),
        "through"
            | "smoke"
            | "dev"
            | "dependency"
            | "dependencies"
            | "in"
            | "how"
            | "should"
            | "an"
            | "a"
            | "the"
            | "and"
            | "or"
            | "before"
            | "after"
            | "changing"
            | "change"
            | "locate"
            | "start"
            | "starts"
            | "from"
            | "which"
            | "what"
            | "where"
            | "when"
            | "why"
            | "owner"
            | "owners"
            | "frontier"
            | "frontiers"
            | "agent"
            | "behavior"
            | "weak"
            | "natural"
            | "term"
            | "terms"
    )
}
