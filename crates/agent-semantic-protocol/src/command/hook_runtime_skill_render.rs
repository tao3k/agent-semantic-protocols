use agent_semantic_hook::{HookActivation, RuntimeProfiles};
use orgize::{
    Org,
    ast::{
        OrgContractEvaluationContext, OrgContractEvaluationScope, OrgContractSeverity,
        evaluate_org_contract_with_context, parse_contract_reference,
        parse_contracts_from_document,
    },
};
use std::path::Path;

const ASP_SKILL_CONTRACT_SOURCE_PATH: &str = "languages/org/contracts/asp.skill.v1.org";
const ASP_SKILL_CONTRACT_ORG: &str =
    include_str!("../../../../languages/org/contracts/asp.skill.v1.org");
const ASP_SKILL_CONTRACT_REFERENCE: &str = "languages/org/contracts/asp.skill.v1.org#asp.skill.v1";
const ASP_SKILL_TEMPLATE_SOURCE_PATH: &str = "languages/org/templates/ASP_ORG_SKILL.org";
const ASP_SKILL_TEMPLATE_HEADING: &str = "** ASP Org";
const ASP_SKILL_ASSERTIONS_HEADING: &str = "** Contract Assertions";

pub(crate) fn render_agent_semantic_protocols_installed_skill(
    _project_root: &Path,
    _org_state_skill_path: &Path,
    _org_artifacts_path: &Path,
    _activation: &HookActivation,
    _runtime_profiles: &RuntimeProfiles,
) -> Result<String, String> {
    render_agent_semantic_protocols_skill_from_contract()
}

pub(crate) fn render_agent_semantic_protocols_plugin_skill(
    _project_root: &Path,
    _org_state_skill_path: &Path,
    _org_artifacts_path: &Path,
    _activation: &HookActivation,
    _runtime_profiles: &RuntimeProfiles,
) -> Result<String, String> {
    render_agent_semantic_protocols_skill_from_contract()
}

fn render_agent_semantic_protocols_skill_from_contract() -> Result<String, String> {
    let rendered = renderable_agent_semantic_protocols_skill_template()?;
    validate_agent_semantic_protocols_skill(&rendered)?;
    Ok(rendered)
}

pub(crate) fn validate_agent_semantic_protocols_skill(rendered_skill: &str) -> Result<(), String> {
    let contract_document = Org::parse(ASP_SKILL_CONTRACT_ORG).document();
    let registry = parse_contracts_from_document(
        &contract_document,
        Some(Path::new(ASP_SKILL_CONTRACT_SOURCE_PATH)),
    );
    let reference = parse_contract_reference(ASP_SKILL_CONTRACT_REFERENCE);
    let contract = registry.resolve(&reference).ok_or_else(|| {
        format!(
            "{ASP_SKILL_CONTRACT_SOURCE_PATH} missing contract `{ASP_SKILL_CONTRACT_REFERENCE}`"
        )
    })?;
    if contract.assertions.is_empty() {
        return Err(format!(
            "{ASP_SKILL_CONTRACT_SOURCE_PATH} contract `{}` has no assertions",
            contract.id
        ));
    }

    let skill_document = Org::parse(rendered_skill).document();
    let evaluation = evaluate_org_contract_with_context(
        &skill_document,
        contract,
        OrgContractEvaluationScope::section(
            "ASP Org",
            vec!["ASP Org".to_string()],
            OrgContractEvaluationScope::document().range(),
        ),
        &OrgContractEvaluationContext::with_source_path(ASP_SKILL_TEMPLATE_SOURCE_PATH),
    );
    let failures = evaluation
        .assertions
        .iter()
        .filter(|assertion| {
            assertion.status.is_failed() && assertion.severity == OrgContractSeverity::Error
        })
        .map(|assertion| {
            format!(
                "- assertion `{}` failed: expected {}, actual {}",
                assertion.assertion_id,
                assertion.expectation.expected_summary(),
                assertion.actual_count
            )
        })
        .collect::<Vec<_>>();

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generated SKILL.org does not match Org contract `{}`:\n{}\nFix {ASP_SKILL_CONTRACT_SOURCE_PATH} or the Rust skill renderer before installing.",
            evaluation.contract_id,
            failures.join("\n")
        ))
    }
}

fn renderable_agent_semantic_protocols_skill_template() -> Result<String, String> {
    let lines = ASP_SKILL_CONTRACT_ORG.lines().collect::<Vec<_>>();
    let template_start = lines
        .iter()
        .position(|line| *line == ASP_SKILL_TEMPLATE_HEADING)
        .ok_or_else(|| {
            format!(
                "{ASP_SKILL_CONTRACT_SOURCE_PATH} missing `{ASP_SKILL_TEMPLATE_HEADING}` template root"
            )
        })?;
    let assertion_start = lines
        .iter()
        .enumerate()
        .skip(template_start + 1)
        .find_map(|(index, line)| (*line == ASP_SKILL_ASSERTIONS_HEADING).then_some(index))
        .ok_or_else(|| {
            format!(
                "{ASP_SKILL_CONTRACT_SOURCE_PATH} missing `{ASP_SKILL_ASSERTIONS_HEADING}` boundary"
            )
        })?;
    if assertion_start <= template_start {
        return Err(format!(
            "{ASP_SKILL_CONTRACT_SOURCE_PATH} assertion boundary must follow the template root"
        ));
    }

    Ok(lines[template_start..assertion_start]
        .iter()
        .map(|line| demote_org_heading_line(line))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn demote_org_heading_line(line: &str) -> String {
    if line.starts_with("**") {
        line[1..].to_string()
    } else {
        line.to_string()
    }
}
