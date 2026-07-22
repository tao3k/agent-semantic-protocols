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

pub fn shell_tokens(command: &str) -> Result<Vec<String>, String> {
    bash_ast_tokens(command).ok_or_else(|| "bash-tree-sitter-parse-failed".to_string())
}

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

fn collect_bash_tokens(node: tree_sitter::Node<'_>, source: &[u8], tokens: &mut Vec<String>) {
    if node.kind() == "command" {
        let mut command_tokens = Vec::new();
        collect_command_words(node, source, &mut command_tokens);
        tokens.extend(command_tokens);
        return;
    }
    if node.child_count() == 0 {
        if let Some(text) = node_text(node, source) {
            if is_separator(&text) {
                tokens.push(text);
            }
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
        } else if let Some(text) = node_text(node, source).map(normalize_shell_word_text) {
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

pub fn command_name(command: &str) -> &str {
    command.rsplit('/').next().unwrap_or(command)
}

pub fn is_separator(token: &str) -> bool {
    matches!(token, "|" | ";" | "&&" | "||" | "&")
}

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

pub fn unwrap_command_stage(tokens: &[String]) -> Result<Vec<Vec<String>>, String> {
    let original_len = tokens.len();
    let tokens = strip_env_assignments(tokens);
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    if tokens.len() != original_len {
        return Ok(vec![tokens.to_vec()]);
    }
    match command_name(&tokens[0]) {
        "timeout" | "gtimeout" => {
            let mut index = 1;
            while index < tokens.len() {
                match tokens[index].as_str() {
                    "--" => {
                        index += 1;
                        break;
                    }
                    "-k" | "--kill-after" | "-s" | "--signal" => {
                        index = (index + 2).min(tokens.len());
                    }
                    "--foreground" | "--preserve-status" | "--verbose" => {
                        index += 1;
                    }
                    option
                        if option.starts_with("--kill-after=")
                            || option.starts_with("--signal=") =>
                    {
                        index += 1;
                    }
                    option if option.starts_with('-') => {
                        index += 1;
                    }
                    _ => break,
                }
            }
            if index < tokens.len() {
                index += 1;
            }
            let command = &tokens[index.min(tokens.len())..];
            return Ok((!command.is_empty())
                .then(|| command.to_vec())
                .into_iter()
                .collect());
        }
        "env" => {
            let command_index = env_command_index(tokens);
            if command_index >= tokens.len() {
                return Ok(Vec::new());
            }
            return Ok(vec![tokens[command_index..].to_vec()]);
        }
        "direnv" => {
            let command_index = if tokens.get(1).is_some_and(|token| token == "exec") {
                let candidate = 2;
                if tokens.get(candidate).is_some_and(|token| {
                    token == "."
                        || token == "--"
                        || token.starts_with('/')
                        || token.starts_with("./")
                        || token.starts_with("../")
                }) {
                    candidate + 1
                } else {
                    candidate
                }
            } else {
                1
            };
            let command = &tokens[command_index.min(tokens.len())..];
            return Ok((!command.is_empty())
                .then(|| command.to_vec())
                .into_iter()
                .collect());
        }
        "rtk" => {
            return unwrap_rtk_stage(tokens);
        }
        "uv" => {
            let command = &tokens[uv_run_command_index(tokens)..];
            return Ok((!command.is_empty())
                .then(|| command.to_vec())
                .into_iter()
                .collect());
        }
        "cargo" => {
            if let Some(index) = tokens.iter().position(|token| token == "--") {
                let command = &tokens[index + 1..];
                return Ok((!command.is_empty())
                    .then(|| command.to_vec())
                    .into_iter()
                    .collect());
            }
        }
        "bash" | "sh" | "zsh" => {
            if let Some(script) = tokens
                .iter()
                .position(|token| matches!(token.as_str(), "-c" | "-lc"))
                .and_then(|index| tokens.get(index + 1))
            {
                return semantic_shell_stages(script);
            }
        }
        _ => {}
    }
    Ok(Vec::new())
}

fn strip_env_assignments(tokens: &[String]) -> &[String] {
    let mut index = 0;
    while tokens
        .get(index)
        .is_some_and(|token| is_env_assignment(token))
    {
        index += 1;
    }
    &tokens[index..]
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn env_command_index(tokens: &[String]) -> usize {
    let mut index = 1;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "-i" | "--ignore-environment" => index += 1,
            "-u" | "--unset" | "-C" | "--chdir" => index += 2,
            value
                if value.starts_with("-u")
                    || value.starts_with("--unset=")
                    || value.starts_with("-C")
                    || value.starts_with("--chdir=")
                    || is_env_assignment(value) =>
            {
                index += 1;
            }
            _ => break,
        }
    }
    index
}

fn unwrap_rtk_stage(tokens: &[String]) -> Result<Vec<Vec<String>>, String> {
    let command_index = tokens
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, token)| (!token.starts_with('-')).then_some(index))
        .unwrap_or(tokens.len());
    let Some(command) = tokens.get(command_index).map(String::as_str) else {
        return Ok(Vec::new());
    };
    match command {
        "run" => {
            if let Some(script) = tokens[command_index + 1..]
                .iter()
                .position(|token| matches!(token.as_str(), "-c" | "-lc"))
                .and_then(|relative_index| tokens.get(command_index + 1 + relative_index + 1))
            {
                return semantic_shell_stages(script);
            }
            Ok(vec![tokens[command_index + 1..].to_vec()])
        }
        "proxy" | "err" | "summary" => Ok(vec![tokens[command_index + 1..].to_vec()]),
        _ => Ok(vec![tokens[command_index..].to_vec()]),
    }
}

fn uv_run_command_index(tokens: &[String]) -> usize {
    let mut index = 2;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "--project" | "--directory" | "--with" | "--python" => index += 2,
            "--frozen" | "--locked" | "--isolated" | "--no-sync" => index += 1,
            value
                if value.starts_with("--project=")
                    || value.starts_with("--directory=")
                    || value.starts_with("--with=")
                    || value.starts_with("--python=") =>
            {
                index += 1;
            }
            _ => break,
        }
    }
    index
}
