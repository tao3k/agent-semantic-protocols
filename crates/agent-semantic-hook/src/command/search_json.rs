use crate::protocol_activation::protocol_activation_manifest::{ActivatedProvider, HookRuntime};

use super::shell::{command_name, is_separator};

pub(crate) fn search_json_route<'a>(
    registry: &'a HookRuntime,
    tokens: &[String],
) -> Option<(&'a ActivatedProvider, Vec<String>)> {
    for provider in &registry.providers {
        let Some((binary_index, command_width)) = provider_command_index(provider, tokens) else {
            continue;
        };
        if tokens.get(binary_index + command_width).map(String::as_str) != Some("search") {
            continue;
        }
        let mut argv = tokens[binary_index + command_width - 1..]
            .iter()
            .take_while(|token| !is_separator(token))
            .filter(|token| token.as_str() != "--json")
            .cloned()
            .collect::<Vec<_>>();
        argv[0] = provider.binary.clone();
        if !argv.iter().any(|arg| arg == "--workspace") {
            if let Some(root_index) = argv.iter().rposition(|arg| arg == ".") {
                argv.splice(
                    root_index..=root_index,
                    ["--workspace".to_string(), ".".to_string()],
                );
            } else {
                argv.extend(["--workspace".to_string(), ".".to_string()]);
            }
        }
        if !argv.iter().any(|arg| arg == "--view") {
            argv.extend(["--view".to_string(), "seeds".to_string()]);
        }
        return Some((provider, argv));
    }
    None
}

fn provider_command_index(
    provider: &ActivatedProvider,
    tokens: &[String],
) -> Option<(usize, usize)> {
    tokens.iter().enumerate().find_map(|(index, token)| {
        if command_name(token) == provider.binary {
            return Some((index, 1));
        }
        if command_name(token) == "asp"
            && tokens
                .get(index + 1)
                .is_some_and(|language| language == &provider.language_id)
        {
            return Some((index, 2));
        }
        None
    })
}
