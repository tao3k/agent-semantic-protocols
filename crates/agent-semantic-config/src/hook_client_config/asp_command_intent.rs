use super::HookClientAspCommandIntentPolicyConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AspCommandIntent {
    Reasoning,
    ExactEvidence,
    InvalidEvidence,
}

impl AspCommandIntent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reasoning => "reasoning",
            Self::ExactEvidence => "exact-evidence",
            Self::InvalidEvidence => "invalid-evidence",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AspCommandRouteId {
    Guide,
    Search(String),
    QueryReasoning,
    QuerySelector,
}

impl AspCommandRouteId {
    pub fn wire_value(&self) -> String {
        match self {
            Self::Guide => "guide".to_string(),
            Self::Search(route) => format!("search-{route}"),
            Self::QueryReasoning => "query-reasoning".to_string(),
            Self::QuerySelector => "query-selector".to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralSelector {
    pub raw: String,
    pub language_id: String,
    pub owner: String,
    pub selector_kind: String,
    pub item_kind: String,
    pub item_name: String,
}

impl StructuralSelector {
    pub fn parse(selector: &str) -> Option<Self> {
        if selector.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return None;
        }
        let (language_id, remainder) = selector.split_once("://")?;
        if !valid_language_id(language_id) || remainder.contains("://") {
            return None;
        }
        let (owner, selector_kind_and_item) = remainder.split_once('#')?;
        let (selector_kind, item) = selector_kind_and_item.split_once('/')?;
        if owner.is_empty() || owner.contains('#') || item.contains('#') {
            return None;
        }
        let mut item_parts = item.split('/');
        let item_kind = item_parts.next()?;
        let item_name = item_parts.next()?;
        if selector_kind.is_empty()
            || item_kind.is_empty()
            || item_name.is_empty()
            || item_parts.clone().any(str::is_empty)
        {
            return None;
        }
        let item_name = std::iter::once(item_name)
            .chain(item_parts)
            .collect::<Vec<_>>()
            .join("/");
        Some(Self {
            raw: selector.to_string(),
            language_id: language_id.to_string(),
            owner: owner.to_string(),
            selector_kind: selector_kind.to_string(),
            item_kind: item_kind.to_string(),
            item_name,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspCommandIntentMatch {
    pub language_id: String,
    pub intent: AspCommandIntent,
    pub route: AspCommandRouteId,
    pub selector: Option<String>,
    pub parsed_selector: Option<StructuralSelector>,
}

pub fn classify_asp_language_command(
    language_id: String,
    tokens: &[String],
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> Option<AspCommandIntentMatch> {
    match tokens.first()?.as_str() {
        "guide" if policy.reasoning.guide_command => Some(AspCommandIntentMatch {
            language_id,
            intent: AspCommandIntent::Reasoning,
            route: AspCommandRouteId::Guide,
            selector: None,
            parsed_selector: None,
        }),
        "search" => classify_search(language_id, tokens, policy),
        "query" => classify_query(language_id, tokens, policy),
        _ => None,
    }
}

fn classify_search(
    language_id: String,
    tokens: &[String],
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> Option<AspCommandIntentMatch> {
    let route_index = if tokens.get(1).map(String::as_str) == Some("--language") {
        3
    } else {
        1
    };
    let route = tokens.get(route_index)?.as_str();
    if !policy
        .reasoning
        .search_routes
        .iter()
        .any(|configured| configured == route)
    {
        return None;
    }
    Some(AspCommandIntentMatch {
        language_id,
        intent: AspCommandIntent::Reasoning,
        route: AspCommandRouteId::Search(route.to_string()),
        selector: None,
        parsed_selector: None,
    })
}

fn classify_query(
    language_id: String,
    tokens: &[String],
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> Option<AspCommandIntentMatch> {
    let selector = option_value(tokens, "--selector").map(str::to_string);
    let parsed_selector = selector.as_deref().and_then(StructuralSelector::parse);

    if tokens.iter().any(|token| {
        policy
            .reasoning
            .query_flags
            .iter()
            .any(|configured| configured == token)
    }) {
        return Some(AspCommandIntentMatch {
            language_id,
            intent: AspCommandIntent::Reasoning,
            route: AspCommandRouteId::QueryReasoning,
            selector,
            parsed_selector,
        });
    }

    let projects_evidence = tokens.iter().any(|token| {
        policy
            .exact_evidence
            .query_projection_flags
            .iter()
            .any(|configured| configured == token)
    }) || option_value(tokens, "--view").is_some_and(|view| {
        policy
            .exact_evidence
            .query_projection_views
            .iter()
            .any(|configured| configured == view)
    });

    if projects_evidence {
        let exact_selector = parsed_selector.as_ref().is_some_and(|parsed| {
            (!policy.exact_evidence.require_same_language || parsed.language_id == language_id)
                && policy
                    .exact_evidence
                    .selector_kinds
                    .iter()
                    .any(|configured| configured == &parsed.selector_kind)
        });
        let cross_language = parsed_selector
            .as_ref()
            .is_some_and(|parsed| parsed.language_id != language_id);
        let intent = if exact_selector {
            AspCommandIntent::ExactEvidence
        } else if policy
            .invalid_evidence
            .reject_projected_query_without_exact_selector
            || (cross_language && policy.invalid_evidence.reject_cross_language_selector)
        {
            AspCommandIntent::InvalidEvidence
        } else {
            AspCommandIntent::Reasoning
        };
        return Some(AspCommandIntentMatch {
            language_id,
            intent,
            route: AspCommandRouteId::QuerySelector,
            selector,
            parsed_selector,
        });
    }

    policy
        .reasoning
        .unprojected_query
        .then_some(AspCommandIntentMatch {
            language_id,
            intent: AspCommandIntent::Reasoning,
            route: AspCommandRouteId::QueryReasoning,
            selector,
            parsed_selector,
        })
}

fn option_value<'a>(tokens: &'a [String], flag: &str) -> Option<&'a str> {
    tokens
        .iter()
        .position(|token| token == flag)
        .and_then(|index| tokens.get(index + 1))
        .map(String::as_str)
}

fn valid_language_id(language_id: &str) -> bool {
    let mut chars = language_id.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '+' | '-' | '.')
        })
}

#[cfg(test)]
#[path = "../../tests/unit/hook_client_config/asp_command_intent.rs"]
mod asp_command_intent_tests;
