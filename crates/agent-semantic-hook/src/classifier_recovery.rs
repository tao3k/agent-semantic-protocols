use std::collections::HashSet;

use crate::{ActivatedProvider, DecisionRoute};

pub(crate) fn source_access_recovery_message(
    reason: &str,
    providers: &[&ActivatedProvider],
    routes: &[DecisionRoute],
    semantic_ast_patch_enabled: bool,
) -> String {
    let mut lines = vec![
        "# ASP Hook Recovery".to_string(),
        String::new(),
        format!("The pre-tool hook blocked `{reason}` on language source."),
        String::new(),
        "## Stop".to_string(),
        "Do not retry `Read`, `cat`, `sed`, `rg`, or source-dump commands on the matched source. The hook runs before the tool and will deny the same raw access again.".to_string(),
        String::new(),
        "## Run Next".to_string(),
    ];
    render_routes(&mut lines, routes);
    let unique_providers = unique_activated_providers(providers);
    render_detected_binaries(&mut lines, &unique_providers);
    render_agent_flow(&mut lines, &unique_providers, semantic_ast_patch_enabled);
    render_rules(&mut lines, &unique_providers, semantic_ast_patch_enabled);
    lines.join("\n")
}

fn render_routes(lines: &mut Vec<String>, routes: &[DecisionRoute]) {
    for route in routes {
        lines.push(String::new());
        lines.push("```sh".to_string());
        lines.push(command_line(&route.argv));
        lines.push("```".to_string());
    }
    if routes.is_empty() {
        lines.push(String::new());
        lines.push("```sh".to_string());
        lines.push("asp guide".to_string());
        lines.push("```".to_string());
    }
}

fn render_detected_binaries(lines: &mut Vec<String>, providers: &[&ActivatedProvider]) {
    if providers.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("## Detected Binaries".to_string());
    for provider in providers {
        lines.push(format!(
            "- language={} provider={} command=`{}` facade=`asp {}`",
            provider.language_id,
            provider.provider_id,
            command_line(&provider_detected_command(provider)),
            provider.language_id
        ));
    }
}

fn render_agent_flow(
    lines: &mut Vec<String>,
    providers: &[&ActivatedProvider],
    semantic_ast_patch_enabled: bool,
) {
    lines.push(String::new());
    lines.push("## Agent Flow".to_string());
    lines.push(
        "Follow this flow after the hook recovery command gives you a frontier or exact locator."
            .to_string(),
    );
    for provider in providers {
        lines.push(String::new());
        lines.push(format!(
            "### {}",
            agent_flow_language_heading(&provider.language_id)
        ));
        if is_document_language(&provider.language_id) {
            render_document_flow(lines, &provider.language_id);
        } else {
            render_source_flow(lines, &provider.language_id, semantic_ast_patch_enabled);
        }
    }
    if providers.is_empty() {
        lines.push(String::new());
        lines.push("### Provider Discovery".to_string());
        lines.push("1. Start from the generic guide.".to_string());
        lines.push("   - `asp guide`".to_string());
        lines.push(
            "2. Inspect active providers, then rerun the language-specific guide.".to_string(),
        );
    }
}

fn render_document_flow(lines: &mut Vec<String>, language_id: &str) {
    lines.push(
        "1. Start from the document guide when you need the provider-owned tool map.".to_string(),
    );
    lines.push(format!("   - `asp {language_id} guide .`"));
    lines.push(format!(
        "2. Query parser-owned document metadata with `asp {language_id} query --term <term> --view metadata .`."
    ));
    lines.push(format!(
        "3. Read exact document content with `asp {language_id} query --selector <path-or-range> --content .`."
    ));
    lines.push("4. Treat stdout from `query --content` as pure document content only.".to_string());
}

fn render_source_flow(
    lines: &mut Vec<String>,
    language_id: &str,
    semantic_ast_patch_enabled: bool,
) {
    lines.push("1. Start from the language guide when you need the agent tool map.".to_string());
    lines.push(format!("   - `asp {language_id} guide .`"));
    lines.push(format!(
        "2. Map the project with `asp {language_id} search prime --view seeds .`."
    ));
    lines.push("3. Choose an owner, query, dependency, test, or syntax profile from the user intent and the seed frontier.".to_string());
    lines.push(format!(
        "4. When you need syntax location, read `asp {language_id} query guide treesitter .`."
    ));
    lines.push(format!(
        "5. Execute `asp {language_id} query --treesitter-query '<pattern>' .` for a capture/frontier result."
    ));
    lines.push("6. Select one exact locator from the frontier.".to_string());
    lines.push(format!(
        "7. Extract pure code with `asp {language_id} query --selector <path-or-range> --treesitter-query '<narrow-pattern>' --code .`."
    ));
    lines.push("8. Treat stdout from `query --code` as pure source code only.".to_string());
    if semantic_ast_patch_enabled {
        lines.push(format!("9. Patch with `apply_patch` for normal edits, or use provider `ast-patch` for structural/mechanical edits after a dry-run receipt; then run `asp {language_id} check --changed .`."));
    } else {
        lines.push(format!("9. Hook config has `experimental.semanticAstPatch.enabled = false`, so patch with `apply_patch`; use provider `ast-patch` only after enabling that config and validating a dry-run receipt, then run `asp {language_id} check --changed .`."));
    }
}

fn render_rules(
    lines: &mut Vec<String>,
    providers: &[&ActivatedProvider],
    semantic_ast_patch_enabled: bool,
) {
    lines.push(String::new());
    lines.push("## Rules".to_string());
    if providers
        .iter()
        .any(|provider| is_document_language(&provider.language_id))
    {
        lines.push("- Document query is parser-owned metadata or exact content; it does not use `search owner` or `--code`.".to_string());
        lines.push(
            "- Query with `--content` is for exact document selector extraction.".to_string(),
        );
    }
    if providers
        .iter()
        .any(|provider| !is_document_language(&provider.language_id))
    {
        lines.push("- Search is for discovery and should not inline code.".to_string());
        lines.push("- Query with `--code` is for exact or unique code extraction.".to_string());
        lines.push(
            "- Tree-sitter query is the syntax base; native parser facts enrich the capture/frontier."
                .to_string(),
        );
    }
    lines.push(
        "- Do not read full guide bodies unless the current step needs that guide.".to_string(),
    );
    lines.push(
        "- Codex and Claude may trigger different hook events, but the recovery route should stay on the same `asp <language>` facade."
            .to_string(),
    );
    if semantic_ast_patch_enabled {
        lines.push(
            "- `ast-patch` is available for structural/mechanical edits after a provider dry-run receipt."
                .to_string(),
        );
    } else {
        lines.push(
            "- `ast-patch` is disabled by hook config; do not route ordinary edits through provider mutation."
                .to_string(),
        );
    }
}

fn unique_activated_providers<'a>(
    providers: &'a [&'a ActivatedProvider],
) -> Vec<&'a ActivatedProvider> {
    let mut seen = HashSet::new();
    providers
        .iter()
        .copied()
        .filter(|provider| {
            seen.insert((provider.language_id.clone(), provider.provider_id.clone()))
        })
        .collect()
}

fn provider_detected_command(provider: &ActivatedProvider) -> Vec<String> {
    if provider.provider_command_prefix.is_empty() {
        vec![provider.binary.clone()]
    } else {
        provider.provider_command_prefix.clone()
    }
}

fn agent_flow_language_heading(language_id: &str) -> String {
    match language_id {
        "rust" => "Rust".to_string(),
        "typescript" => "TypeScript".to_string(),
        "python" => "Python".to_string(),
        "julia" => "Julia".to_string(),
        "org" => "Org Document".to_string(),
        "md" => "Markdown Document".to_string(),
        other => other.to_string(),
    }
}

fn is_document_language(language_id: &str) -> bool {
    matches!(language_id, "org" | "md")
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
