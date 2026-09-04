use super::AgentAction;
use super::AgentActionKind;
use super::AgentActionSubjectKind;
use super::HostInvocationKind;
use super::SemanticCapability;
use super::SemanticCapabilityEvidence;
use crate::HookRuntime;
use crate::tool_action::OperationIntent;
use crate::tool_action::ToolAction;

/// Projects host and parser facts into the Action IR before any rule is evaluated.
///
/// Rules consume this envelope; they never manufacture semantic actions. A bare
/// registered-source operand remains Unknown until a Host fact, shell redirection,
/// or trusted Reader probe establishes its filesystem permission. Profile and
/// extension matching remain independent subject axes for the Rule DSL.
pub(crate) fn project_agent_action(
    registry: &HookRuntime,
    action: &ToolAction,
    match_paths: Option<&[String]>,
    structured_source_operands: Option<&[String]>,
) -> AgentAction {
    let (mut agent_action, command_stages, behavior_facts) =
        action.derive_agent_action_with_shell_facts();

    let mut subject_paths = structured_source_operands.unwrap_or_default().to_vec();
    let mut candidate_subject_paths = command_stages
        .iter()
        .flat_map(|stage| stage.words().iter().skip(1).cloned())
        .collect::<Vec<_>>();
    for path in match_paths.unwrap_or_default() {
        if !candidate_subject_paths.contains(path) {
            candidate_subject_paths.push(path.clone());
        }
    }
    let projected_subject_paths = if action.operation == OperationIntent::ShellCommand {
        crate::source_selector::project_shell_subject_paths(registry, &candidate_subject_paths)
    } else {
        candidate_subject_paths
    };
    for operand in projected_subject_paths {
        if !subject_paths.contains(&operand) {
            subject_paths.push(operand);
        }
    }
    for subject in behavior_facts
        .iter()
        .filter_map(|fact| fact.subject.as_ref())
    {
        if !subject_paths.contains(subject) {
            subject_paths.push(subject.clone());
        }
    }
    subject_paths.dedup();

    let subjects = crate::source_selector::derive_agent_action_subjects(registry, &subject_paths);
    if agent_action.host.action == HostInvocationKind::Execute {
        let probed_reader_subject =
            crate::reader_probe::observed_reader_subject(&action.host_payload);
        for subject in subjects
            .iter()
            .filter(|subject| subject.kind == AgentActionSubjectKind::RegisteredLanguageSource)
        {
            if probed_reader_subject == Some(subject.value.as_str()) {
                agent_action.add_filesystem_permission(
                    crate::action_ir::FilesystemPermissionFact::new(
                        crate::action_ir::FilesystemPermissionKind::Read,
                        crate::action_ir::FilesystemPermissionSource::ReaderProbe,
                        Some(subject.value.clone()),
                    ),
                );
            }
            let subject_has_explicit_permission = agent_action
                .filesystem_permissions
                .iter()
                .any(|permission| permission.subject.as_deref() == Some(subject.value.as_str()));
            if !subject_has_explicit_permission {
                agent_action.add_capability(SemanticCapability {
                    action: AgentActionKind::Unknown,
                    evidence: SemanticCapabilityEvidence::RegisteredSourceOperand,
                });
            }
        }
    }
    agent_action.subjects = subjects;
    agent_action
}
