//! Search lexical replay matching for cache artifact reuse.

use serde_json::Value;

/// Request facts needed to decide whether a lexical search packet can replay.
pub struct SearchLexicalReplayRequest<'a> {
    pub is_search_method: bool,
    pub forwarded_args: &'a [String],
}

/// Return whether a semantic search packet matches one lexical search request.
pub fn search_lexical_packet_matches_request(
    packet: &Value,
    request: SearchLexicalReplayRequest<'_>,
) -> bool {
    if !request.is_search_method || !has_only_replay_safe_lexical_args(request.forwarded_args) {
        return false;
    }
    if string_field(packet, "schemaId") != Some("agent.semantic-protocols.semantic-search-packet") {
        return false;
    }
    if string_field(packet, "method") != Some("search/lexical") {
        return false;
    }
    string_field(packet, "query") == request_search_lexical_query(request.forwarded_args)
}

fn has_only_replay_safe_lexical_args(args: &[String]) -> bool {
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--workspace" | "--view" => {
                if args.get(index + 1).is_none() {
                    return false;
                }
                index += 2;
            }
            arg if arg.starts_with("--workspace=") || arg.starts_with("--view=") => {
                index += 1;
            }
            arg if arg.starts_with('-') => return false,
            _ => index += 1,
        }
    }
    true
}

fn request_search_lexical_query(forwarded_args: &[String]) -> Option<&str> {
    forwarded_args.windows(2).find_map(|window| {
        (window[0] == "lexical" && !window[1].starts_with('-') && window[1] != ".")
            .then_some(window[1].as_str())
    })
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}
