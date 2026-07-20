use agent_semantic_config::AspCommandRouteId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DirectSourceReadScope {
    None,
    Bounded(usize),
    Broad,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PromptSearchFeedback {
    RepeatPrimeBeforePipe,
    RepeatSearchPipe,
    DirectSourceReadAfterPipe,
}

pub(super) struct PromptSearchFeedbackRequest<'a> {
    pub route: &'a AspCommandRouteId,
    pub same_language: bool,
    pub saw_pipe: bool,
    pub repeated_pipe: bool,
    pub direct_source_read: DirectSourceReadScope,
    pub bounded_read_max_lines: usize,
}

#[must_use]
pub(super) fn evaluate_prompt_search_feedback(
    request: PromptSearchFeedbackRequest<'_>,
) -> Option<PromptSearchFeedback> {
    let search_route = match request.route {
        AspCommandRouteId::Search(route) => Some(route.as_str()),
        _ => None,
    };
    if !request.saw_pipe {
        if request.same_language && search_route == Some("prime") {
            return Some(PromptSearchFeedback::RepeatPrimeBeforePipe);
        }
        return None;
    }
    if request.same_language && request.repeated_pipe && search_route == Some("pipe") {
        return Some(PromptSearchFeedback::RepeatSearchPipe);
    }
    match request.direct_source_read {
        DirectSourceReadScope::Broad => Some(PromptSearchFeedback::DirectSourceReadAfterPipe),
        DirectSourceReadScope::Bounded(lines) if lines > request.bounded_read_max_lines => {
            Some(PromptSearchFeedback::DirectSourceReadAfterPipe)
        }
        DirectSourceReadScope::None | DirectSourceReadScope::Bounded(_) => None,
    }
}
pub(super) const LOW_PRIORITY_DIRECT_SOURCE_READ_MAX_LINES: usize = 80;
