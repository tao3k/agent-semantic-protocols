use orgize::Org;
use orgize::rowan::ast::AstNode;
use orgize::syntax_ast::{Headline, PropertyDrawer};
use std::{fs, path::Path};

pub(super) fn materialize_contract_capture_args(
    args: &[String],
    contract_id: &str,
    template_path: Option<&Path>,
) -> Result<Vec<String>, String> {
    let mut capture_args = args.to_vec();
    match contract_id {
        "agent.task.v1" => {
            materialize_task_contract_capture(args, &mut capture_args, template_path)?
        }
        "agent.plan.v1" => {
            materialize_plan_contract_capture(args, &mut capture_args, template_path)?
        }
        _ => {}
    }
    Ok(capture_args)
}

fn materialize_task_contract_capture(
    args: &[String],
    capture_args: &mut Vec<String>,
    template_path: Option<&Path>,
) -> Result<(), String> {
    materialize_from_template(args, capture_args, template_path, "agent.task.v1")?;
    Ok(())
}

fn materialize_plan_contract_capture(
    args: &[String],
    capture_args: &mut Vec<String>,
    template_path: Option<&Path>,
) -> Result<(), String> {
    validate_plan_target_file(args)?;
    validate_plan_title(args)?;
    let materialized =
        materialize_from_template(args, capture_args, template_path, "agent.plan.v1")?;
    if !has_flag(args, "--kind") {
        capture_args.extend(["--kind".to_string(), "task".to_string()]);
    }
    ensure_plan_title_progress_cookie(capture_args, &materialized.progress_cookies)?;
    Ok(())
}

fn validate_plan_title(args: &[String]) -> Result<(), String> {
    let Some(title) = flag_value(args, "--title").map(str::trim) else {
        return Err(
            "agent.plan.v1 capture requires `--title` with the real task title; do not rely on session id or template placeholders for recall"
                .to_string(),
        );
    };
    if title.is_empty()
        || ["agent session plan", "plan title", "<plan_title>"]
            .iter()
            .any(|placeholder| title.eq_ignore_ascii_case(placeholder))
    {
        return Err(
            "agent.plan.v1 --title must be a task-specific recall title, not a generic session label or template placeholder"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_plan_target_file(args: &[String]) -> Result<(), String> {
    let Some(target_file) = flag_value(args, "--target-file") else {
        return Ok(());
    };
    let target_path = Path::new(target_file);
    let filename = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "agent.plan.v1 --target-file must end with an Org filename".to_string())?;
    if !filename.starts_with("agent-plan-") || !filename.ends_with(".org") {
        return Err(
            "agent.plan.v1 --target-file filename must match `agent-plan-*.org`".to_string(),
        );
    }
    let parent_parts: Vec<&str> = target_path
        .parent()
        .into_iter()
        .flat_map(|parent| parent.components())
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    let under_org_flow_plans = parent_parts
        .len()
        .checked_sub(3)
        .map(|start| {
            parent_parts[start] == "org"
                && parent_parts[start + 1] == "flow"
                && parent_parts[start + 2] == "plans"
        })
        .unwrap_or(false);
    if !under_org_flow_plans {
        return Err(
            "agent.plan.v1 --target-file must be stored under an `org/flow/plans/` path"
                .to_string(),
        );
    }
    Ok(())
}

fn materialize_from_template(
    args: &[String],
    capture_args: &mut Vec<String>,
    template_path: Option<&Path>,
    contract_id: &str,
) -> Result<TemplateMaterialization, String> {
    let template_path = template_path.ok_or_else(|| {
        format!(
            "ASP Org template for `{contract_id}` was not found; run `asp org capture init` to sync templates"
        )
    })?;
    let CaptureTemplate {
        tags,
        properties,
        body,
        progress_cookies,
    } = CaptureTemplate::read(template_path, args)?;
    if !has_flag(args, "--kind") {
        capture_args.extend(["--kind".to_string(), "task".to_string()]);
    }
    for tag in tags {
        ensure_capture_tag(capture_args, &tag);
    }
    for (key, value) in properties {
        ensure_capture_property(capture_args, &key, &value);
    }
    if !has_flag(args, "--body") {
        capture_args.extend(["--body".to_string(), body]);
    }
    Ok(TemplateMaterialization { progress_cookies })
}

struct TemplateMaterialization {
    progress_cookies: Vec<String>,
}

struct CaptureTemplate {
    tags: Vec<String>,
    properties: Vec<(String, String)>,
    body: String,
    progress_cookies: Vec<String>,
}

impl CaptureTemplate {
    fn read(path: &Path, args: &[String]) -> Result<Self, String> {
        let source = fs::read_to_string(path).map_err(|error| {
            format!(
                "failed to read Org capture template {}: {error}",
                path.display()
            )
        })?;
        let org = Org::parse(&source);
        let headline = org.first_node::<Headline>().ok_or_else(|| {
            format!(
                "Org capture template {} does not contain a headline",
                path.display()
            )
        })?;
        let drawer = org.first_node::<PropertyDrawer>().ok_or_else(|| {
            format!(
                "Org capture template {} does not contain a property drawer",
                path.display()
            )
        })?;
        let dynamic = TemplateDynamicValues::from_args(args);
        let tags = headline.tags().map(|tag| tag.to_string()).collect();
        let progress_cookies = progress_cookies_from_org(&org);
        let properties = drawer
            .iter()
            .filter_map(|(key, value)| {
                let key = key.to_string();
                (!key.starts_with("TEMPLATE_") && key != "CONTRACT_ORG").then(|| {
                    let value = dynamic.apply(value.as_ref());
                    (key, value)
                })
            })
            .collect();
        let body_start = text_size_to_usize(drawer.syntax().text_range().end());
        let body_end = text_size_to_usize(headline.syntax().text_range().end());
        let body = source
            .get(body_start..body_end)
            .unwrap_or_default()
            .trim_start_matches(['\r', '\n'])
            .to_string();
        Ok(Self {
            tags,
            properties,
            body: dynamic.apply(&body),
            progress_cookies,
        })
    }
}

fn progress_cookies_from_org(org: &Org) -> Vec<String> {
    org.document()
        .progress_stats_records()
        .into_iter()
        .next()
        .map(|record| {
            record
                .statistic_cookies
                .into_iter()
                .map(|cookie| cookie.raw)
                .collect()
        })
        .unwrap_or_default()
}

struct TemplateDynamicValues {
    id: String,
    objective: String,
}

impl TemplateDynamicValues {
    fn from_args(args: &[String]) -> Self {
        Self {
            id: default_plan_id(args),
            objective: default_plan_objective(args),
        }
    }

    fn apply(&self, value: &str) -> String {
        value
            .replace("agent-plan-id", &self.id)
            .replace("agent-task-id", &self.id)
            .replace("durable outcome this plan records", &self.objective)
            .replace("Record the task intent.", &self.objective)
            .replace("stable-source-ref", "capture-request")
            .replace(
                "stable work boundary or design reference",
                "current-task-boundary",
            )
    }
}

fn text_size_to_usize(value: orgize::TextSize) -> usize {
    u32::from(value) as usize
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window.first().is_some_and(|arg| arg == flag))
        .and_then(|window| window.get(1))
        .map(String::as_str)
}

fn ensure_capture_tag(args: &mut Vec<String>, tag: &str) {
    if args
        .windows(2)
        .any(|window| window.first().is_some_and(|arg| arg == "--tag") && window[1] == tag)
    {
        return;
    }
    args.extend(["--tag".to_string(), tag.to_string()]);
}

fn ensure_capture_property(args: &mut Vec<String>, key: &str, value: &str) {
    let property_prefix = format!("{key}=");
    if args.windows(2).any(|window| {
        window.first().is_some_and(|arg| arg == "--property")
            && window[1].starts_with(&property_prefix)
    }) {
        return;
    }
    args.extend(["--property".to_string(), format!("{key}={value}")]);
}

fn ensure_plan_title_progress_cookie(
    args: &mut [String],
    template_cookies: &[String],
) -> Result<(), String> {
    let Some(title) = mutable_flag_value(args, "--title") else {
        return Ok(());
    };
    if title_has_progress_cookie(title) {
        return Ok(());
    }
    if template_cookies.is_empty() {
        return Err(
            "agent.plan.v1 template must provide native Org progress cookies in its headline"
                .to_string(),
        );
    }
    title.push(' ');
    title.push_str(&template_cookies.join(" "));
    Ok(())
}

fn title_has_progress_cookie(title: &str) -> bool {
    let source = format!("* TODO {title}\n");
    !progress_cookies_from_org(&Org::parse(source)).is_empty()
}

fn mutable_flag_value<'a>(args: &'a mut [String], flag: &str) -> Option<&'a mut String> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get_mut(index + 1)
}

fn default_plan_objective(args: &[String]) -> String {
    flag_value(args, "--title")
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or("durable outcome this plan records")
        .to_string()
}

fn default_plan_id(args: &[String]) -> String {
    flag_value(args, "--title")
        .map(slugify_plan_id)
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "agent-plan".to_string())
}

fn slugify_plan_id(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}
