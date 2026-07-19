pub(super) struct ExhaustedAspCommandBudget {
    pub(super) command_count: usize,
    pub(super) max_commands: usize,
}

pub(super) fn exhausted_asp_command_budget(
    command_count: usize,
) -> Option<ExhaustedAspCommandBudget> {
    let max_commands = prompt_asp_command_budget()?;
    (command_count >= max_commands).then_some(ExhaustedAspCommandBudget {
        command_count,
        max_commands,
    })
}

pub(super) fn asp_command_budget_message(budget: &ExhaustedAspCommandBudget) -> String {
    [
        format!(
            "ASP hook denied ASP command budget exhaustion: {}/{} commands already completed.",
            budget.command_count, budget.max_commands
        ),
        "Answer from the existing ASP frontier, recommendedNext, nextCommand, owner, locator, and output metadata instead of running more commands.".to_string(),
        String::new(),
        "## Rules".to_string(),
        "Do not run more ASP commands in this prompt after the budget is exhausted.".to_string(),
        "Do not switch to raw shell reads; use the evidence already returned by ASP.".to_string(),
    ]
    .join("\n")
}

fn prompt_asp_command_budget() -> Option<usize> {
    std::env::var("ASP_HOOK_MAX_ASP_COMMANDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}
