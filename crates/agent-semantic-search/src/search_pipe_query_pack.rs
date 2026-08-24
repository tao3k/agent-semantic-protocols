#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPipeTermRole {
    Context,
    Concept,
    Symbol,
    Literal,
    DiagnosticCode,
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
    explicit_role: Option<SearchPipeTermRole>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPipeLanguageId<'a>(pub(crate) &'a str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPipeQueryText<'a>(pub(crate) &'a str);

impl<'a> SearchPipeLanguageId<'a> {
    pub const fn from_language_id(language_id: Self) -> Self {
        language_id
    }
}

impl<'a> SearchPipeQueryText<'a> {
    pub const fn from_query(query: &'a str) -> Self {
        Self(query)
    }
}

impl SearchPipeTermRole {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Concept => "concept",
            Self::Symbol => "symbol",
            Self::Literal => "literal",
            Self::DiagnosticCode => "diagnostic-code",
        }
    }
}

#[must_use]
pub fn search_pipe_query_clauses<'a>(
    request: SearchPipeQueryClausesRequest<'a, SearchPipeQueryPackDescriptor<'a>>,
) -> Vec<SearchPipeQueryClause> {
    let language_id = request.language_id;
    let query = request.query.as_str();
    let query_pack_descriptor = request.query_pack_descriptor;
    let explicit_clauses = query
        .split('|')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .map(|raw_clause| SearchPipeQueryClause {
            terms: search_pipe_query_terms(language_id.as_str(), raw_clause, query_pack_descriptor),
        })
        .filter(|clause| !clause.terms.is_empty())
        .collect::<Vec<_>>();
    if query.contains('|') || explicit_clauses.is_empty() {
        return explicit_clauses;
    }

    let mut owner_terms = Vec::new();
    let mut symbol_terms = Vec::new();
    let mut concept_terms = Vec::new();
    for term in explicit_clauses.into_iter().flat_map(|clause| clause.terms) {
        if term.role == SearchPipeTermRole::Context {
            continue;
        }
        if is_owner_seed_token(&term.raw) {
            owner_terms.push(term);
        } else if term.role == SearchPipeTermRole::Symbol {
            symbol_terms.push(term);
        } else {
            concept_terms.push(term);
        }
    }
    [owner_terms, symbol_terms, concept_terms]
        .into_iter()
        .filter(|terms| !terms.is_empty())
        .map(|terms| SearchPipeQueryClause { terms })
        .collect()
}

pub struct SearchPipeQueryClausesRequest<'a, QueryPackDescriptor> {
    language_id: SearchPipeLanguageId<'a>,
    query: SearchPipeQueryText<'a>,
    query_pack_descriptor: QueryPackDescriptor,
}

pub struct SearchPipeQueryPackDescriptorMissing;

#[derive(Clone, Copy, Debug)]
pub struct SearchPipeSemanticFactsIntentAxis<'a> {
    pub axis: &'a str,
    pub terms: &'a [String],
    pub roles: &'a [SearchPipeTermRole],
}

#[derive(Clone, Copy, Debug)]
pub struct SearchPipeSemanticFactsDescriptor<'a> {
    pub descriptor_id: &'a str,
    pub descriptor_version: &'a str,
    pub intent_axes: &'a [SearchPipeSemanticFactsIntentAxis<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPipeQueryPackTermRoleOverride<'a> {
    pub term: &'a str,
    pub role: SearchPipeTermRole,
    pub case_sensitive: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SearchPipeQueryPackClause<'a> {
    pub terms: &'a [String],
    pub roles: &'a [SearchPipeTermRole],
    pub intent_axes: &'a [String],
}

#[derive(Clone, Copy, Debug)]
pub struct SearchPipeQueryPackRecipe<'a> {
    pub recipe_id: &'a str,
    pub trigger_terms: &'a [String],
    pub trigger_match: &'a str,
    pub clauses: &'a [SearchPipeQueryPackClause<'a>],
}

#[derive(Clone, Copy, Debug)]
pub struct SearchPipeQueryPackDescriptor<'a> {
    pub descriptor_id: &'a str,
    pub descriptor_version: &'a str,
    pub language_id: &'a str,
    pub term_role_overrides: &'a [SearchPipeQueryPackTermRoleOverride<'a>],
    pub recipes: &'a [SearchPipeQueryPackRecipe<'a>],
}

pub struct SearchPipeSemanticFactsIntentDecision {
    pub requested: bool,
    pub descriptor_id: String,
    pub descriptor_version: String,
    pub matched_axes: Vec<String>,
    pub matched_terms: Vec<String>,
}

impl<'a> SearchPipeLanguageId<'a> {
    #[must_use]
    pub const fn new(raw: &'a str) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> SearchPipeQueryText<'a> {
    #[must_use]
    pub const fn new(raw: &'a str) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> SearchPipeQueryClausesRequest<'a, SearchPipeQueryPackDescriptorMissing> {
    #[must_use]
    pub const fn new(
        language_id: SearchPipeLanguageId<'a>,
        query: SearchPipeQueryText<'a>,
    ) -> Self {
        Self {
            language_id,
            query,
            query_pack_descriptor: SearchPipeQueryPackDescriptorMissing,
        }
    }

    #[must_use]
    pub const fn with_query_pack_descriptor(
        self,
        query_pack_descriptor: SearchPipeQueryPackDescriptor<'a>,
    ) -> SearchPipeQueryClausesRequest<'a, SearchPipeQueryPackDescriptor<'a>> {
        SearchPipeQueryClausesRequest {
            language_id: self.language_id,
            query: self.query,
            query_pack_descriptor,
        }
    }
}

#[must_use]
pub fn search_pipe_query_clause_texts<'a>(
    request: SearchPipeQueryClausesRequest<'a, SearchPipeQueryPackDescriptor<'a>>,
) -> Vec<String> {
    search_pipe_query_clauses(request)
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
pub fn search_pipe_typed_query_terms(
    language_id: SearchPipeLanguageId<'_>,
    query: SearchPipeQueryText<'_>,
    query_pack_descriptor: SearchPipeQueryPackDescriptor<'_>,
) -> Vec<SearchPipeQueryTerm> {
    let clauses = search_pipe_query_clauses(
        SearchPipeQueryClausesRequest::new(language_id, query)
            .with_query_pack_descriptor(query_pack_descriptor),
    );
    search_pipe_unique_query_terms(&clauses)
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
    descriptor: SearchPipeQueryPackDescriptor<'_>,
    context_terms: &[String],
    owner_seed_terms: &[String],
    concept_terms: &[String],
) -> Option<String> {
    if descriptor.descriptor_id.is_empty()
        || descriptor.descriptor_version != "1"
        || owner_seed_terms.len() < 2
    {
        return None;
    }
    Some(build_next_query_pack_hint(
        descriptor,
        context_terms,
        owner_seed_terms,
        concept_terms,
    ))
}

fn build_next_query_pack_hint(
    descriptor: SearchPipeQueryPackDescriptor<'_>,
    context_terms: &[String],
    owner_seed_terms: &[String],
    concept_terms: &[String],
) -> String {
    let mut clauses = vec![owner_seed_terms.join(" ")];
    let observed_terms = context_terms
        .iter()
        .chain(owner_seed_terms)
        .chain(concept_terms)
        .map(|term| term.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    let recipe_clauses = matching_recipe_clauses(descriptor.recipes, &observed_terms);
    append_unique_clauses(&mut clauses, recipe_clauses);
    if clauses.len() == 1 && !context_terms.is_empty() {
        clauses.push(context_terms.join(" "));
    } else if clauses.len() == 1 && !concept_terms.is_empty() {
        clauses.push(concept_terms.join(" "));
    }
    clauses.join("|")
}

fn matching_recipe_clauses(
    recipes: &[SearchPipeQueryPackRecipe<'_>],
    observed_terms: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    recipes
        .iter()
        .filter(|recipe| recipe_matches_observed_terms(recipe, observed_terms))
        .flat_map(|recipe| recipe.clauses)
        .map(|clause| clause.terms.join(" "))
        .filter(|clause| !clause.is_empty())
        .collect()
}

fn recipe_matches_observed_terms(
    recipe: &SearchPipeQueryPackRecipe<'_>,
    observed_terms: &std::collections::BTreeSet<String>,
) -> bool {
    let trigger_terms = recipe
        .trigger_terms
        .iter()
        .map(|trigger| trigger.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    match recipe.trigger_match {
        "all" => trigger_terms.is_subset(observed_terms),
        _ => !trigger_terms.is_disjoint(observed_terms),
    }
}

fn append_unique_clauses(clauses: &mut Vec<String>, candidates: Vec<String>) {
    let mut seen = clauses
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    clauses.extend(
        candidates
            .into_iter()
            .filter(|clause| seen.insert(clause.clone())),
    );
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

fn term_role(
    language_id: &str,
    raw: &str,
    query_pack_descriptor: SearchPipeQueryPackDescriptor<'_>,
) -> SearchPipeTermRole {
    if let Some(role_override) = (query_pack_descriptor.language_id == language_id)
        .then_some(query_pack_descriptor)
        .and_then(|descriptor| {
            descriptor.term_role_overrides.iter().find(|role_override| {
                if role_override.case_sensitive {
                    role_override.term == raw
                } else {
                    role_override.term.eq_ignore_ascii_case(raw)
                }
            })
        })
    {
        return role_override.role;
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

fn search_pipe_query_terms(
    language_id: &str,
    raw_clause: &str,
    query_pack_descriptor: SearchPipeQueryPackDescriptor<'_>,
) -> Vec<SearchPipeQueryTerm> {
    raw_clause
        .split(|character: char| character == ',' || character.is_whitespace())
        .flat_map(query_token_fragments)
        .map(|fragment| SearchPipeQueryTerm {
            raw: fragment.raw.clone(),
            lower: fragment.raw.to_ascii_lowercase(),
            role: if let Some(role) = fragment.explicit_role {
                role
            } else if fragment.force_symbol {
                SearchPipeTermRole::Symbol
            } else {
                term_role(language_id, &fragment.raw, query_pack_descriptor)
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
    if let Some((prefix, value)) = trimmed.split_once(':') {
        let explicit_role = match prefix {
            "literal" => Some(SearchPipeTermRole::Literal),
            "diagnostic" | "diagnostic-code" => Some(SearchPipeTermRole::DiagnosticCode),
            _ => None,
        };
        if let Some(explicit_role) = explicit_role {
            let value = trim_query_token(value);
            if value.is_empty() || !has_ascii_query_signal(value) {
                return Vec::new();
            }
            return vec![QueryTokenFragment {
                raw: value.to_string(),
                force_symbol: false,
                explicit_role: Some(explicit_role),
            }];
        }
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
        explicit_role: None,
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

fn is_owner_seed_token(raw: &str) -> bool {
    search_pipe_is_path_like_token(raw) || is_package_like_token(raw)
}

fn is_package_like_token(raw: &str) -> bool {
    raw.matches('-').count() >= 2
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
pub fn search_pipe_semantic_facts_intent(
    language_id: SearchPipeLanguageId<'_>,
    query: SearchPipeQueryText<'_>,
    query_pack_descriptor: SearchPipeQueryPackDescriptor<'_>,
    descriptor: SearchPipeSemanticFactsDescriptor<'_>,
) -> SearchPipeSemanticFactsIntentDecision {
    if query
        .as_str()
        .split_whitespace()
        .any(search_pipe_is_path_like_token)
    {
        return empty_semantic_facts_intent(descriptor);
    }
    resolve_semantic_facts_intent(language_id, query, query_pack_descriptor, descriptor)
}

fn empty_semantic_facts_intent(
    descriptor: SearchPipeSemanticFactsDescriptor<'_>,
) -> SearchPipeSemanticFactsIntentDecision {
    SearchPipeSemanticFactsIntentDecision {
        requested: false,
        descriptor_id: descriptor.descriptor_id.to_owned(),
        descriptor_version: descriptor.descriptor_version.to_owned(),
        matched_axes: Vec::new(),
        matched_terms: Vec::new(),
    }
}

fn resolve_semantic_facts_intent(
    language_id: SearchPipeLanguageId<'_>,
    query: SearchPipeQueryText<'_>,
    query_pack_descriptor: SearchPipeQueryPackDescriptor<'_>,
    descriptor: SearchPipeSemanticFactsDescriptor<'_>,
) -> SearchPipeSemanticFactsIntentDecision {
    let clauses = search_pipe_query_clauses(
        SearchPipeQueryClausesRequest::new(
            SearchPipeLanguageId::new((language_id).as_str()),
            SearchPipeQueryText::new((query).as_str()),
        )
        .with_query_pack_descriptor(query_pack_descriptor),
    );
    let terms = search_pipe_unique_query_terms(&clauses);
    let mut matched_axes = Vec::new();
    let mut matched_terms = Vec::new();
    let mut seen_axes = std::collections::BTreeSet::new();
    let mut seen_terms = std::collections::BTreeSet::new();
    let has_symbol_anchor = terms
        .iter()
        .any(|term| matches!(term.role, SearchPipeTermRole::Symbol));
    let indexed_axes = descriptor
        .intent_axes
        .iter()
        .map(|axis| {
            let role_mask = axis
                .roles
                .iter()
                .fold(0_u8, |mask, role| mask | semantic_fact_role_bit(*role));
            let term_index = axis
                .terms
                .iter()
                .map(|term| term.to_ascii_lowercase())
                .collect::<std::collections::BTreeSet<_>>();
            (axis, role_mask, term_index)
        })
        .collect::<Vec<_>>();
    for term in &terms {
        for (intent_axis, role_mask, term_index) in &indexed_axes {
            if *role_mask != 0 && *role_mask & semantic_fact_role_bit(term.role) == 0 {
                continue;
            }
            if !term_index.contains(term.lower.as_str()) {
                continue;
            }
            if seen_axes.insert(intent_axis.axis) {
                matched_axes.push(intent_axis.axis.to_owned());
            }
            if seen_terms.insert(term.lower.as_str()) {
                matched_terms.push(term.lower.clone());
            }
        }
    }
    let requested = matched_terms.len() >= 2 || (matched_terms.len() == 1 && has_symbol_anchor);
    SearchPipeSemanticFactsIntentDecision {
        requested,
        descriptor_id: descriptor.descriptor_id.to_owned(),
        descriptor_version: descriptor.descriptor_version.to_owned(),
        matched_axes,
        matched_terms,
    }
}

const fn semantic_fact_role_bit(role: SearchPipeTermRole) -> u8 {
    match role {
        SearchPipeTermRole::Context => 1 << 0,
        SearchPipeTermRole::Concept => 1 << 1,
        SearchPipeTermRole::Symbol => 1 << 2,
        SearchPipeTermRole::Literal => 1 << 3,
        SearchPipeTermRole::DiagnosticCode => 1 << 4,
    }
}
