fn shell_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, '|' | ';' | '&') => {
                push_token(&mut tokens, &mut current);
                if ch == '&' && chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push("&&".to_string());
                } else {
                    tokens.push(ch.to_string());
                }
            }
            (None, c) if c.is_whitespace() => push_token(&mut tokens, &mut current),
            (None, c) => current.push(c),
        }
    }
    push_token(&mut tokens, &mut current);
    tokens
}

pub(crate) fn semantic_shell_tokens(command: &str) -> Vec<String> {
    split_command_stages(shell_tokens(command))
        .into_iter()
        .flat_map(|stage| unwrap_command_stage(&stage))
        .collect()
}

pub(super) fn command_name(command: &str) -> &str {
    command.rsplit('/').next().unwrap_or(command)
}

pub(super) fn is_separator(token: &str) -> bool {
    matches!(token, "|" | ";" | "&&" | "&")
}

fn split_command_stages(tokens: Vec<String>) -> Vec<Vec<String>> {
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

fn unwrap_command_stage(tokens: &[String]) -> Vec<String> {
    let tokens = strip_env_assignments(tokens);
    if tokens.is_empty() {
        return Vec::new();
    }
    match command_name(&tokens[0]) {
        "env" => return unwrap_command_stage(&tokens[env_command_index(tokens)..]),
        "direnv" if tokens.get(1).map(String::as_str) == Some("exec") => {
            if tokens.len() > 3 {
                return unwrap_command_stage(&tokens[3..]);
            }
        }
        "rtk" => return unwrap_rtk_stage(tokens),
        "uv" if tokens.get(1).map(String::as_str) == Some("run") => {
            return unwrap_command_stage(&tokens[uv_run_command_index(tokens)..]);
        }
        "cargo" => {
            if let Some(index) = tokens.iter().position(|token| token == "--") {
                return unwrap_command_stage(&tokens[index + 1..]);
            }
        }
        "bash" | "sh" | "zsh" => {
            if let Some(script) = tokens
                .iter()
                .position(|token| matches!(token.as_str(), "-c" | "-lc"))
                .and_then(|index| tokens.get(index + 1))
            {
                return semantic_shell_tokens(script);
            }
        }
        _ => {}
    }
    tokens.to_vec()
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

fn unwrap_rtk_stage(tokens: &[String]) -> Vec<String> {
    let command_index = tokens
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, token)| (!token.starts_with('-')).then_some(index))
        .unwrap_or(tokens.len());
    let Some(command) = tokens.get(command_index).map(String::as_str) else {
        return Vec::new();
    };
    match command {
        "run" => {
            if let Some(script) = tokens[command_index + 1..]
                .iter()
                .position(|token| matches!(token.as_str(), "-c" | "-lc"))
                .and_then(|offset| tokens.get(command_index + 1 + offset + 1))
            {
                return semantic_shell_tokens(script);
            }
            unwrap_command_stage(&tokens[command_index + 1..])
        }
        "proxy" | "err" | "summary" => unwrap_command_stage(&tokens[command_index + 1..]),
        _ => unwrap_command_stage(&tokens[command_index..]),
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

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}
