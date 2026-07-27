macro_rules! shell_kind_matcher {
    ($name:ident, [$($kind:literal),+ $(,)?]) => {
        fn $name(kind: &str) -> bool {
            matches!(kind, $($kind)|+)
        }
    };
}

const NESTED_STAGE_SEPARATOR: &str = ";";

shell_kind_matcher!(
    is_command_word_node,
    [
        "command_name",
        "word",
        "string",
        "raw_string",
        "concatenation",
        "file_descriptor",
        "number",
        "simple_expansion",
        "variable_assignment",
    ]
);

shell_kind_matcher!(
    is_nested_command_stage_node,
    ["command_substitution", "process_substitution", "subshell"]
);

/// Parse a shell command into parser-owned semantic tokens.
pub fn shell_tokens(command: &str) -> Result<Vec<String>, String> {
    bash_ast_tokens(command).ok_or_else(|| "bash-tree-sitter-parse-failed".to_string())
}

/// Parse a Bash command into normalized AST tokens within the parser budget.
pub(crate) fn bash_ast_tokens(command: &str) -> Option<Vec<String>> {
    // Bash syntax is parsed once by the command-match owner.
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_bash::LANGUAGE.into();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(command, None)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    let mut tokens = Vec::new();
    collect_bash_tokens(root, command.as_bytes(), &mut tokens);
    (!tokens.is_empty()).then_some(tokens)
}

pub(crate) fn bash_heredoc_literal_candidates(command: &str) -> Vec<String> {
    if !command.contains("<<") {
        return Vec::new();
    }

    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_bash::LANGUAGE.into();
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(command, None) else {
        return Vec::new();
    };
    let root = tree.root_node();
    if root.has_error() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    collect_heredoc_literal_candidates(root, command.as_bytes(), &mut candidates);
    candidates
}

fn collect_heredoc_literal_candidates(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    candidates: &mut Vec<String>,
) {
    if node.kind() == "heredoc_body" {
        let Some(body) = node_text(node, source) else {
            return;
        };
        if !collect_patch_header_paths(&body, candidates) {
            collect_quoted_literals(&body, candidates);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_heredoc_literal_candidates(child, source, candidates);
    }
}

fn collect_patch_header_paths(text: &str, candidates: &mut Vec<String>) -> bool {
    let mut found = false;
    for line in text.lines() {
        let line = line.trim();
        for prefix in [
            "*** Add File:",
            "*** Update File:",
            "*** Delete File:",
            "*** Move to:",
        ] {
            let Some(path) = line.strip_prefix(prefix) else {
                continue;
            };
            let path = path.trim();
            if !path.is_empty() {
                candidates.push(path.to_string());
                found = true;
            }
        }
    }
    found
}

/// Extract unique paths declared by apply-patch headers.
pub fn apply_patch_header_paths(patch: &str) -> Vec<String> {
    apply_patch_header_paths_impl(patch)
}

fn apply_patch_header_paths_impl(patch: &str) -> Vec<String> {
    let mut paths = Vec::new();
    collect_patch_header_paths(patch, &mut paths);
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn collect_quoted_literals(text: &str, candidates: &mut Vec<String>) {
    let mut quote = None;
    let mut literal = String::new();
    let mut escaped = false;

    for character in text.chars() {
        match quote {
            Some(delimiter) => {
                if escaped {
                    literal.push(character);
                    escaped = false;
                } else if character == '\\' && delimiter == '"' {
                    escaped = true;
                } else if character == delimiter {
                    if !literal.is_empty() {
                        candidates.push(std::mem::take(&mut literal));
                    }
                    quote = None;
                } else {
                    literal.push(character);
                }
            }
            None if matches!(character, '\'' | '"') => quote = Some(character),
            None => {}
        }
    }
}

/// Extract quoted literal candidates from an already parsed command token.
pub(crate) fn quoted_literal_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    collect_quoted_literals(text, &mut candidates);
    candidates
}

fn collect_bash_tokens(node: tree_sitter::Node<'_>, source: &[u8], tokens: &mut Vec<String>) {
    if node.kind() == "command" {
        let mut command_tokens = Vec::new();
        collect_command_words(node, source, &mut command_tokens);
        tokens.extend(command_tokens);
        return;
    }
    if node.child_count() == 0 {
        let Some(text) = node_text(node, source) else {
            return;
        };
        if is_separator(&text) {
            tokens.push(text);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_bash_tokens(child, source, tokens);
    }
}

fn collect_command_words(node: tree_sitter::Node<'_>, source: &[u8], tokens: &mut Vec<String>) {
    if is_nested_command_stage_node(node.kind()) {
        push_nested_stage_separator(tokens);
        collect_bash_tokens(node, source, tokens);
        push_nested_stage_separator(tokens);
        return;
    }
    if is_command_word_node(node.kind()) {
        if node_contains_nested_command_stage(node) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_command_words(child, source, tokens);
            }
        } else {
            let Some(text) = node_text(node, source).map(normalize_shell_word_text) else {
                return;
            };
            if !text.is_empty() {
                tokens.push(text);
            }
        }
        return;
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    for child in children {
        collect_command_words(child, source, tokens);
    }
}

fn node_contains_nested_command_stage(node: tree_sitter::Node<'_>) -> bool {
    if is_nested_command_stage_node(node.kind()) {
        return true;
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    children.into_iter().any(node_contains_nested_command_stage)
}

fn push_nested_stage_separator(tokens: &mut Vec<String>) {
    if tokens.last().is_some_and(|token| is_separator(token)) {
        return;
    }
    tokens.push(NESTED_STAGE_SEPARATOR.to_string());
}

fn node_text(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source).ok().map(str::to_string)
}

fn normalize_shell_word_text(text: String) -> String {
    let stripped = text
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .or_else(|| {
            text.strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
        .unwrap_or(&text);
    stripped
        .replace("\\ ", " ")
        .replace("\\'", "'")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

/// Parse a nested shell script without collapsing its independent stages.
pub fn semantic_shell_stages(command: &str) -> Result<Vec<Vec<String>>, String> {
    let tokens = shell_tokens(command)?;
    Ok(split_command_stages(tokens)
        .into_iter()
        .filter(|stage| !stage.is_empty())
        .collect())
}

/// Return the basename of a normalized command token.
pub fn command_name(command: &str) -> &str {
    command.rsplit('/').next().unwrap_or(command)
}

/// Return whether a normalized token separates shell command stages.
pub fn is_separator(token: &str) -> bool {
    matches!(token, "|" | ";" | "&&" | "||" | "&")
}

/// Split normalized tokens into executable and separator stages.
pub fn split_command_stages(tokens: Vec<String>) -> Vec<Vec<String>> {
    let mut stages = Vec::new();
    let mut stage = Vec::new();
    for token in tokens {
        if is_separator(&token) {
            if !stage.is_empty() {
                stages.push(std::mem::take(&mut stage));
            }
            stages.push(vec![token]);
        } else {
            stage.push(token);
        }
    }
    if !stage.is_empty() {
        stages.push(stage);
    }
    stages
}
