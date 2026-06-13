use super::command::semantic_shell_tokens;

pub(crate) fn line_range_source_paths(command: &str) -> Vec<String> {
    for paths in [
        piped_sed_line_range_source_paths(command),
        sed_line_range_source_paths(command),
        awk_line_range_source_paths(command),
        head_tail_line_range_source_paths(command),
        tail_head_line_range_source_paths(command),
        head_line_range_source_paths(command),
    ] {
        if !paths.is_empty() {
            return paths;
        }
    }
    Vec::new()
}

fn sed_line_range_source_paths(command: &str) -> Vec<String> {
    let raw_tokens = command_tokens(command);
    if !raw_tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "sed" | "gsed"))
    {
        return Vec::new();
    }
    let Some((start_line, end_line)) = raw_tokens
        .iter()
        .skip(1)
        .find_map(|token| parse_sed_line_range(token))
    else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    append_ranged_source_paths(
        &mut paths,
        &source_paths_from_raw_tokens(&raw_tokens[1..]),
        start_line,
        end_line,
    );
    paths
}

fn piped_sed_line_range_source_paths(command: &str) -> Vec<String> {
    let segments = pipeline_segments(command);
    if segments.len() < 2 {
        return Vec::new();
    }
    let Some((start_line, end_line)) = segments
        .iter()
        .rev()
        .find_map(|segment| parse_sed_tokens_line_range(segment))
    else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for segment in segments.iter().take(segments.len() - 1) {
        append_ranged_source_paths(
            &mut paths,
            &source_paths_from_raw_tokens(segment),
            start_line,
            end_line,
        );
    }
    paths
}

fn awk_line_range_source_paths(command: &str) -> Vec<String> {
    let raw_tokens = command_tokens(command);
    if !raw_tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "awk" | "gawk"))
    {
        return Vec::new();
    }
    let Some((start_line, end_line)) = parse_awk_nr_line_range(command) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    append_ranged_source_paths(
        &mut paths,
        &source_paths_from_raw_tokens(&raw_tokens[1..]),
        start_line,
        end_line,
    );
    paths
}

fn head_line_range_source_paths(command: &str) -> Vec<String> {
    let raw_tokens = command_tokens(command);
    let Some((line_count, token_index)) = parse_head_line_count(&raw_tokens) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    append_ranged_source_paths(
        &mut paths,
        &source_paths_from_raw_tokens(&raw_tokens[token_index..]),
        1,
        line_count,
    );
    paths
}

fn head_tail_line_range_source_paths(command: &str) -> Vec<String> {
    let segments = pipeline_segments(command);
    if segments.len() != 2 {
        return Vec::new();
    }
    let head_tokens = &segments[0];
    let tail_tokens = &segments[1];
    let Some((head_count, head_path_index)) = parse_head_line_count(head_tokens) else {
        return Vec::new();
    };
    let Some((tail_count, _tail_path_index)) = parse_tail_line_count(tail_tokens) else {
        return Vec::new();
    };
    if tail_count > head_count {
        return Vec::new();
    }
    let start_line = head_count - tail_count + 1;
    let mut paths = Vec::new();
    append_ranged_source_paths(
        &mut paths,
        &source_paths_from_raw_tokens(&head_tokens[head_path_index..]),
        start_line,
        head_count,
    );
    paths
}

fn tail_head_line_range_source_paths(command: &str) -> Vec<String> {
    let segments = pipeline_segments(command);
    if segments.len() != 2 {
        return Vec::new();
    }
    let tail_tokens = &segments[0];
    let head_tokens = &segments[1];
    let Some((start_line, tail_path_index)) = parse_tail_start_line(tail_tokens) else {
        return Vec::new();
    };
    let Some((head_count, _head_path_index)) = parse_head_line_count(head_tokens) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    append_ranged_source_paths(
        &mut paths,
        &source_paths_from_raw_tokens(&tail_tokens[tail_path_index..]),
        start_line,
        start_line + head_count - 1,
    );
    paths
}

fn parse_sed_tokens_line_range(tokens: &[String]) -> Option<(usize, usize)> {
    if !tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "sed" | "gsed"))
    {
        return None;
    }
    tokens
        .iter()
        .skip(1)
        .find_map(|token| parse_sed_line_range(token))
}

fn parse_sed_line_range(token: &str) -> Option<(usize, usize)> {
    let token = token.trim_matches(|ch| matches!(ch, '\'' | '"'));
    let body = token.strip_suffix('p').unwrap_or(token);
    let (start, end) = body.split_once(',')?;
    let start_line = start.parse::<usize>().ok()?;
    let end_line = end.parse::<usize>().ok()?;
    if start_line == 0 || end_line == 0 {
        return None;
    }
    Some((start_line.min(end_line), start_line.max(end_line)))
}

fn parse_awk_nr_line_range(command: &str) -> Option<(usize, usize)> {
    let compact = command
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if let Some(line) = number_after(&compact, "NR==") {
        return Some((line, line));
    }
    let start_line = number_after(&compact, "NR>=")
        .or_else(|| number_after(&compact, "NR>").map(|line| line + 1))?;
    let end_line = number_after(&compact, "NR<=")
        .or_else(|| number_after(&compact, "NR<").and_then(|line| line.checked_sub(1)))?;
    if start_line == 0 || end_line == 0 {
        return None;
    }
    Some((start_line.min(end_line), start_line.max(end_line)))
}

fn number_after(text: &str, marker: &str) -> Option<usize> {
    let start = text.find(marker)? + marker.len();
    let digits = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn parse_head_line_count(tokens: &[String]) -> Option<(usize, usize)> {
    if !tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "head" | "ghead"))
    {
        return None;
    }
    parse_count_option(tokens, 1)
}

fn parse_tail_line_count(tokens: &[String]) -> Option<(usize, usize)> {
    if !tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "tail" | "gtail"))
    {
        return None;
    }
    let (count, index) = parse_count_option(tokens, 1)?;
    if count == 0 {
        return None;
    }
    Some((count, index))
}

fn parse_tail_start_line(tokens: &[String]) -> Option<(usize, usize)> {
    if !tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "tail" | "gtail"))
    {
        return None;
    }
    let (start_line, index) = parse_signed_count_option(tokens, 1, '+')?;
    if start_line == 0 {
        return None;
    }
    Some((start_line, index))
}

fn parse_count_option(tokens: &[String], start_index: usize) -> Option<(usize, usize)> {
    if let Some(option) = tokens.get(start_index) {
        if let Some(count) = option.strip_prefix("-n") {
            if count.is_empty() {
                let value = tokens.get(start_index + 1)?;
                return Some((value.parse().ok()?, start_index + 2));
            }
            return Some((count.parse().ok()?, start_index + 1));
        }
        if option.starts_with('-')
            && option[1..]
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            return Some((option[1..].parse().ok()?, start_index + 1));
        }
    }
    None
}

fn parse_signed_count_option(
    tokens: &[String],
    start_index: usize,
    sign: char,
) -> Option<(usize, usize)> {
    if let Some(option) = tokens.get(start_index) {
        if let Some(count) = option.strip_prefix("-n") {
            if count.is_empty() {
                let value = tokens.get(start_index + 1)?;
                return Some((value.strip_prefix(sign)?.parse().ok()?, start_index + 2));
            }
            return Some((count.strip_prefix(sign)?.parse().ok()?, start_index + 1));
        }
        if let Some(count) = option.strip_prefix(sign) {
            return Some((count.parse().ok()?, start_index + 1));
        }
    }
    None
}

fn append_ranged_source_paths(
    paths: &mut Vec<String>,
    source_paths: &[String],
    start_line: usize,
    end_line: usize,
) {
    for path in source_paths {
        if is_exact_source_path_selector(path) {
            push_unique_path(paths, format!("{path}:{start_line}:{end_line}"));
        }
    }
}

fn command_tokens(command: &str) -> Vec<String> {
    semantic_shell_tokens(command)
}

fn pipeline_segments(command: &str) -> Vec<Vec<String>> {
    command_tokens(command)
        .split(|token| token == "|")
        .filter(|segment| !segment.is_empty())
        .map(<[String]>::to_vec)
        .collect()
}

fn source_paths_from_raw_tokens(tokens: &[String]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut token_index = 0;
    while let Some(token) = tokens.get(token_index) {
        if token.starts_with('-') || parse_sed_line_range(token).is_some() {
            token_index += 1;
            continue;
        }
        let (token, consumed) = unescape_joined_path_token(tokens, token_index);
        token_index += consumed;
        let token = token.trim_matches(|ch| matches!(ch, '\'' | '"' | '`'));
        if is_embedded_source_path_candidate(token) {
            push_unique_path(&mut paths, token);
            continue;
        }
        for path in embedded_source_path_candidates(token) {
            push_unique_path(&mut paths, path);
        }
    }
    paths
}

fn unescape_joined_path_token(tokens: &[String], start_index: usize) -> (String, usize) {
    let mut token = tokens[start_index].clone();
    let mut consumed = 1;
    while token.ends_with('\\') {
        let Some(next) = tokens.get(start_index + consumed) else {
            break;
        };
        token.pop();
        token.push(' ');
        token.push_str(next);
        consumed += 1;
    }
    (token, consumed)
}

fn embedded_source_path_candidates(token: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut current = String::new();
    for ch in token.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '/' | '.' | '_' | '-' | '*' | ':' | '~' | '[' | ']' | '{' | '}'
            )
        {
            current.push(ch);
            continue;
        }
        if is_embedded_source_path_candidate(&current) {
            push_unique_path(&mut paths, current.clone());
        }
        current.clear();
    }
    paths
}

fn is_embedded_source_path_candidate(path: &str) -> bool {
    let path = path.trim_matches(|ch| matches!(ch, '\'' | '"' | '`'));
    if path.is_empty() {
        return false;
    }
    source_extension(path).is_some()
}

fn source_extension(path: &str) -> Option<&str> {
    for extension in [
        ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs", ".jl",
    ] {
        if path.ends_with(extension) || path.contains(&format!("{extension}:")) {
            return Some(extension);
        }
    }
    None
}

fn is_exact_source_path_selector(path: &str) -> bool {
    !path.contains('*') && !path.contains('{') && !path.contains('[')
}

fn push_unique_path(paths: &mut Vec<String>, path: impl Into<String>) {
    let path = path.into();
    if !paths.contains(&path) {
        paths.push(path);
    }
}
