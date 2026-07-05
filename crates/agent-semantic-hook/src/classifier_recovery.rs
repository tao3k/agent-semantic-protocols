use std::borrow::Cow;

use crate::hook_recovery_prompt::CompiledRecoveryPromptConfig;
use crate::{ActivatedProvider, DecisionRoute};

pub const HOOK_TRIGGER_PROMPT_FILE_NAME: &str = "hook_trigger_prompt.md";

const HOOK_TRIGGER_PROMPT_MD: &str = include_str!("../templates/hook_trigger_prompt.md");
const MANAGED_BEGIN: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-BEGIN -->";
const MANAGED_END: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-END -->";
const USER_EXTENSIONS_BEGIN: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-BEGIN -->";
const USER_EXTENSIONS_END: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-END -->";

pub(crate) fn source_access_recovery_message(
    platform: &str,
    reason: &str,
    _providers: &[&ActivatedProvider],
    routes: &[DecisionRoute],
    _semantic_ast_patch_enabled: bool,
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> String {
    default_hook_trigger_prompt_message_for_platform(platform, reason, routes, recovery_prompt)
}

pub fn hook_trigger_prompt_document() -> &'static str {
    HOOK_TRIGGER_PROMPT_MD
}

pub fn default_hook_trigger_prompt_message(reason: &str, routes: &[DecisionRoute]) -> String {
    let recovery_prompt = CompiledRecoveryPromptConfig::default();
    default_hook_trigger_prompt_message_for_platform("codex", reason, routes, &recovery_prompt)
}

fn default_hook_trigger_prompt_message_for_platform(
    platform: &str,
    reason: &str,
    routes: &[DecisionRoute],
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> String {
    let document = recovery_prompt.template().unwrap_or(HOOK_TRIGGER_PROMPT_MD);
    render_hook_trigger_prompt_document_for_platform(
        document,
        platform,
        reason,
        routes,
        recovery_prompt,
    )
}

pub fn render_hook_trigger_prompt_document(
    document: &str,
    reason: &str,
    routes: &[DecisionRoute],
) -> String {
    let recovery_prompt = CompiledRecoveryPromptConfig::default();
    render_hook_trigger_prompt_document_for_platform(
        document,
        "codex",
        reason,
        routes,
        &recovery_prompt,
    )
}

pub fn materialize_hook_trigger_prompt_agent_flow_for_client(
    document: &str,
    client: &str,
) -> String {
    let platform = effective_recovery_platform(client);
    document.replace("{agent_flow}", default_agent_flow_markdown(&platform))
}

fn render_hook_trigger_prompt_document_for_platform(
    document: &str,
    platform: &str,
    reason: &str,
    routes: &[DecisionRoute],
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> String {
    let managed = section_body(document, MANAGED_BEGIN, MANAGED_END).unwrap_or(document);
    let mut rendered =
        render_hook_trigger_prompt_template(managed, platform, reason, routes, recovery_prompt);
    if let Some(user_extensions) = runtime_user_extensions(document) {
        rendered.push_str("\n\n");
        rendered.push_str(user_extensions);
    }
    rendered
}

pub fn merge_hook_trigger_prompt_document(existing: Option<&str>) -> String {
    let Some(existing) = existing else {
        return HOOK_TRIGGER_PROMPT_MD.to_string();
    };
    let Some(user_extensions) = section_body(existing, USER_EXTENSIONS_BEGIN, USER_EXTENSIONS_END)
    else {
        return HOOK_TRIGGER_PROMPT_MD.to_string();
    };
    replace_section(
        HOOK_TRIGGER_PROMPT_MD,
        USER_EXTENSIONS_BEGIN,
        USER_EXTENSIONS_END,
        user_extensions,
    )
    .unwrap_or_else(|| HOOK_TRIGGER_PROMPT_MD.to_string())
}

fn render_hook_trigger_prompt_template(
    template: &str,
    platform: &str,
    reason: &str,
    routes: &[DecisionRoute],
    recovery_prompt: &CompiledRecoveryPromptConfig,
) -> String {
    let platform = effective_recovery_platform(platform);
    let agent_flow = recovery_prompt
        .agent_flow_for(&platform)
        .unwrap_or_else(|| default_agent_flow_markdown(&platform));
    template
        .trim_matches('\n')
        .replace("{reason}", reason)
        .replace("{agent_flow}", agent_flow)
        .replace("{routes}", &routes_markdown(routes))
}

fn effective_recovery_platform(platform: &str) -> Cow<'_, str> {
    let platform = platform.trim();
    if !platform.is_empty()
        && !platform.eq_ignore_ascii_case("unknown")
        && !platform.eq_ignore_ascii_case("auto")
    {
        return Cow::Borrowed(platform);
    }

    match (
        non_empty_env("CODEX_THREAD_ID"),
        non_empty_env("CLAUDE_CODE_SESSION_ID")
            .or_else(|| non_empty_env("CLAUDE_CODE_REMOTE_SESSION_ID")),
    ) {
        (Some(_), None) => Cow::Borrowed("codex"),
        (None, Some(_)) => Cow::Borrowed("claude"),
        _ => Cow::Borrowed(platform),
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn default_agent_flow_markdown(platform: &str) -> &'static str {
    if platform.eq_ignore_ascii_case("codex") {
        "Codex: delegate the lookup to `spawn_agent` with the ASP route below when available; otherwise run the route directly and return one compact `[asp-search-subagent]` graph-route receipt with schema/intent/route/state/evidence/next. Do not return source bodies, snippets, or line-range selectors."
    } else if platform.eq_ignore_ascii_case("claude") {
        "Claude: run the ASP route below directly and return one compact `[asp-search-subagent]` graph-route receipt with schema/intent/route/state/evidence/next. Do not return source bodies, snippets, or line-range selectors."
    } else {
        "Run the ASP route below directly and return one compact `[asp-search-subagent]` graph-route receipt with schema/intent/route/state/evidence/next. Do not return source bodies, snippets, or line-range selectors."
    }
}

fn runtime_user_extensions(document: &str) -> Option<&str> {
    let user_extensions = section_body(document, USER_EXTENSIONS_BEGIN, USER_EXTENSIONS_END)?;
    let has_visible_content = user_extensions.lines().any(|line| {
        let trimmed = line.trim();
        !(trimmed.is_empty() || trimmed.starts_with("<!--") && trimmed.ends_with("-->"))
    });
    has_visible_content.then_some(user_extensions)
}

fn section_body<'a>(document: &'a str, begin: &str, end: &str) -> Option<&'a str> {
    let start = document.find(begin)? + begin.len();
    let rest = &document[start..];
    let end = rest.find(end)?;
    Some(rest[..end].trim_matches('\n'))
}

fn replace_section(document: &str, begin: &str, end: &str, body: &str) -> Option<String> {
    let start = document.find(begin)?;
    let body_start = start + begin.len();
    let rest = &document[body_start..];
    let body_end = body_start + rest.find(end)?;

    let mut output = String::new();
    output.push_str(&document[..body_start]);
    output.push('\n');
    output.push_str(body.trim_matches('\n'));
    output.push('\n');
    output.push_str(&document[body_end..]);
    Some(output)
}

fn routes_markdown(routes: &[DecisionRoute]) -> String {
    if routes.is_empty() {
        return "```sh\nasp guide\n```".to_string();
    }
    routes
        .iter()
        .map(|route| format!("```sh\n{}\n```", command_line(&route.argv)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn command_line(argv: &[String]) -> String {
    let argv = display_argv(argv);
    argv.iter()
        .map(|arg| shell_quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_argv(argv: &[String]) -> Vec<String> {
    if !uses_agent_facade_workspace_positional(argv) {
        return argv.to_vec();
    }

    let workspace = argv[argv.len() - 1].clone();
    let mut rendered = argv[..argv.len() - 1].to_vec();
    let insert_at = rendered
        .iter()
        .position(|arg| arg == "--view")
        .unwrap_or(rendered.len());
    rendered.insert(insert_at, "--workspace".to_string());
    rendered.insert(insert_at + 1, workspace);
    rendered
}

fn uses_agent_facade_workspace_positional(argv: &[String]) -> bool {
    if argv.len() < 4 || argv.iter().any(|arg| arg == "--workspace") {
        return false;
    }
    if !matches!(argv.first().map(String::as_str), Some("asp")) {
        return false;
    }
    if !matches!(argv.get(2).map(String::as_str), Some("query" | "search")) {
        return false;
    }
    argv.last()
        .is_some_and(|arg| !arg.is_empty() && !arg.starts_with('-'))
}

fn shell_quote_arg(arg: &str) -> String {
    if arg.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/' | ':')
    }) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}
