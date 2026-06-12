use crate::{ActivatedProvider, DecisionRoute};

pub const HOOK_TRIGGER_PROMPT_FILE_NAME: &str = "hook_trigger_prompt.md";

const HOOK_TRIGGER_PROMPT_MD: &str = include_str!("../templates/hook_trigger_prompt.md");
const MANAGED_BEGIN: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-BEGIN -->";
const MANAGED_END: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-END -->";
const USER_EXTENSIONS_BEGIN: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-BEGIN -->";
const USER_EXTENSIONS_END: &str = "<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-END -->";

pub(crate) fn source_access_recovery_message(
    reason: &str,
    _providers: &[&ActivatedProvider],
    routes: &[DecisionRoute],
    _semantic_ast_patch_enabled: bool,
) -> String {
    default_hook_trigger_prompt_message(reason, routes)
}

pub fn hook_trigger_prompt_document() -> &'static str {
    HOOK_TRIGGER_PROMPT_MD
}

pub fn default_hook_trigger_prompt_message(reason: &str, routes: &[DecisionRoute]) -> String {
    render_hook_trigger_prompt_document(HOOK_TRIGGER_PROMPT_MD, reason, routes)
}

pub fn render_hook_trigger_prompt_document(
    document: &str,
    reason: &str,
    routes: &[DecisionRoute],
) -> String {
    let managed = section_body(document, MANAGED_BEGIN, MANAGED_END).unwrap_or(document);
    let mut rendered = render_hook_trigger_prompt_template(managed, reason, routes);
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
    reason: &str,
    routes: &[DecisionRoute],
) -> String {
    template
        .trim_matches('\n')
        .replace("{reason}", reason)
        .replace("{routes}", &routes_markdown(routes))
}

fn runtime_user_extensions(document: &str) -> Option<&str> {
    let user_extensions = section_body(document, USER_EXTENSIONS_BEGIN, USER_EXTENSIONS_END)?;
    let has_visible_content = user_extensions.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !(trimmed.starts_with("<!--") && trimmed.ends_with("-->"))
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
    argv.iter()
        .map(|arg| shell_quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote_arg(arg: &str) -> String {
    if arg.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/' | ':')
    }) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}
