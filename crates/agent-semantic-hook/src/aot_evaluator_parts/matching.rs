// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Matcher normalization and rule-composition predicates for the AOT evaluator.

use super::{BorrowedHookPayload, CompiledDecisionRule, CompiledHookPolicyBundle};

pub(super) fn normalized_agent_eq(left: &str, right: &str) -> bool {
    left.chars()
        .map(|character| if character == '-' { '_' } else { character })
        .eq(right
            .chars()
            .map(|character| if character == '-' { '_' } else { character }))
}

pub(super) fn rule_conditions_match(
    rule: &CompiledDecisionRule<'_>,
    generation: &CompiledHookPolicyBundle<'_>,
    payload: &BorrowedHookPayload<'_>,
    shell_command: Option<&str>,
    shell_stages: Option<&[agent_semantic_shell_parser::CommandStage]>,
) -> Result<bool, String> {
    let has_conditions = !rule.argv_prefix_any.is_empty()
        || !rule.command_contains_any.is_empty()
        || !rule.argv_token_all.is_empty()
        || !rule.argv_source_glob_any.is_empty()
        || !rule.command_any.is_empty()
        || !rule.process_environment_assignment_any.is_empty()
        || !rule.path_glob_any.is_empty()
        || rule.argv_workspace_regular_file
        || rule.argv_structured_document_file
        || rule.structured_projection.is_some();
    if !has_conditions {
        return Ok(true);
    }
    if payload.tool_name != "Bash" {
        return Ok(rule.path_glob_any.is_empty() || payload.tool_input.get().contains("*** "));
    }
    let Some(command) = shell_command else {
        return Ok(false);
    };
    let Some(stages) = shell_stages else {
        return Ok(false);
    };
    let words = stages
        .iter()
        .filter(|stage| !stage.is_separator())
        .flat_map(|stage| stage.words().iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    if !rule.process_environment_assignment_any.is_empty()
        && !agent_semantic_shell_parser::command_stages_match_process_environment_assignment(
            stages,
            &rule.process_environment_assignment_any,
        )
    {
        return Ok(false);
    }
    if !rule.argv_prefix_any.is_empty()
        && !rule
            .argv_prefix_any
            .iter()
            .any(|pattern| match_argv_pattern(stages, pattern, &generation.registered_languages))
    {
        return Ok(false);
    }
    if !rule.command_contains_any.is_empty()
        && !rule
            .command_contains_any
            .iter()
            .any(|needle| command.contains(needle))
    {
        return Ok(false);
    }
    if !rule.argv_token_all.is_empty()
        && !rule
            .argv_token_all
            .iter()
            .all(|required| words.iter().any(|word| word == required))
    {
        return Ok(false);
    }
    let source_paths = stages
        .iter()
        .filter(|stage| !stage.is_separator())
        .flat_map(agent_semantic_shell_parser::command_stage_source_paths)
        .collect::<Vec<_>>();
    if !rule.argv_source_glob_any.is_empty()
        && !source_paths.iter().any(|path| {
            rule.argv_source_glob_any
                .iter()
                .any(|pattern| simple_path_glob_matches(pattern, path))
        })
    {
        return Ok(false);
    }
    if !rule.path_glob_any.is_empty()
        && !source_paths.iter().any(|path| {
            rule.path_glob_any
                .iter()
                .any(|pattern| simple_path_glob_matches(pattern, path))
        })
    {
        return Ok(false);
    }
    if !rule.command_any.is_empty()
        && !stages
            .iter()
            .filter_map(|stage| stage.executable())
            .any(|executable| {
                let basename = executable.rsplit('/').next().unwrap_or(executable);
                rule.command_any.iter().any(|command| basename == *command)
            })
    {
        return Ok(false);
    }
    if rule.argv_structured_document_file
        && !source_paths
            .iter()
            .any(|path| has_any_extension(path, &["json", "toml"]))
    {
        return Ok(false);
    }
    if rule.argv_workspace_regular_file {
        let cwd = std::path::Path::new(payload.cwd.unwrap_or("."));
        if !source_paths.iter().any(|path| {
            let path = std::path::Path::new(path);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            };
            resolved.is_file()
        }) {
            return Ok(false);
        }
    }
    if let Some(spec) = &rule.structured_projection {
        let classification =
            agent_semantic_shell_parser::structured::classify_single_bounded_path_command(
                command,
                agent_semantic_shell_parser::structured::BoundedPathCommandSpec {
                    binary: spec.binary,
                    optional_subcommand_any: &spec.optional_subcommand_any,
                    option_any: &spec.option_any,
                    option_value_arity: &spec.option_value_arity,
                    max_slice_items: spec.max_slice_items,
                },
            );
        if !matches!(
            classification,
            agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedPath { .. }
                | agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedScalarPredicate { .. }
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn match_argv_pattern(
    stages: &[agent_semantic_shell_parser::CommandStage],
    pattern: &[&str],
    registered_languages: &[&str],
) -> bool {
    if let Some(index) = pattern
        .iter()
        .position(|token| *token == "<registered-language>")
    {
        return registered_languages.iter().any(|language| {
            let mut concrete = pattern
                .iter()
                .map(|token| (*token).to_owned())
                .collect::<Vec<_>>();
            concrete[index] = (*language).to_owned();
            agent_semantic_shell_parser::command_stages_match_wrapped_prefix(stages, &concrete)
                .routes_protected()
        });
    }
    let concrete = pattern
        .iter()
        .map(|token| (*token).to_owned())
        .collect::<Vec<_>>();
    agent_semantic_shell_parser::command_stages_match_wrapped_prefix(stages, &concrete)
        .routes_protected()
}

fn simple_path_glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == "**" || pattern == "*" {
        return true;
    }
    pattern
        .rsplit_once("*.")
        .is_some_and(|(_, extension)| has_any_extension(path, &[extension]))
        || pattern == path
}

fn has_any_extension(path: &str, extensions: &[&str]) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extensions.contains(&extension))
}

pub fn host_matcher_matches_tool_name(host_matcher: &str, tool_name: &str) -> bool {
    host_matcher == tool_name
        || (tool_name == "apply_patch" && matches!(host_matcher, "apply_patch" | "Edit" | "Write"))
        || (tool_name == "spawn_agent" && matches!(host_matcher, "spawn_agent" | "Agent"))
}

pub(super) fn configured_matcher_matches(configured: &str, host_matcher: &str) -> bool {
    configured == host_matcher
        || configured
            .strip_prefix('^')
            .and_then(|matcher| matcher.strip_suffix('$'))
            == Some(host_matcher)
}
