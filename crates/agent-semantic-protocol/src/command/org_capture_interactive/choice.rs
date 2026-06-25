use orgize::Org;
use std::{fs, path::Path};

pub(in crate::command) struct AgentInteractiveChoice {
    pub(in crate::command) id: String,
    method: String,
    stage: String,
    target: Option<String>,
    create: String,
    info: String,
    categories: String,
    entries: Vec<AgentInteractiveChoiceEntry>,
}

struct AgentInteractiveChoiceEntry {
    number: String,
    id: String,
    contract: Option<String>,
    full: String,
    use_if: String,
}

impl AgentInteractiveChoice {
    pub(in crate::command) fn read(path: &Path, expected_stage: &str) -> Result<Self, String> {
        let source = fs::read_to_string(path)
            .map_err(|error| format!("failed to read Org contract {}: {error}", path.display()))?;
        let org = Org::parse(&source);
        for record in org.document().source_block_records() {
            if record.language.as_deref() != Some("org-contract") {
                continue;
            }
            let block_type = record
                .header_args
                .iter()
                .find(|arg| arg.key == "type")
                .and_then(|arg| arg.value.as_deref());
            if block_type != Some("agent-interactive") {
                continue;
            }
            let choice = Self::parse(&record.value)?;
            if choice.stage == expected_stage {
                return Ok(choice);
            }
        }
        Err(format!(
            "Org contract {} must declare `#+BEGIN_SRC org-contract :type agent-interactive` with `method: choice` and `stage: {expected_stage}`",
            path.display()
        ))
    }

    fn parse(value: &str) -> Result<Self, String> {
        let mut id = None;
        let mut method = None;
        let mut stage = None;
        let mut target = None;
        let mut create = None;
        let mut info = None;
        let mut categories = None;
        let mut in_details = false;
        let mut entries = Vec::new();
        for line in value.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if in_details && line.starts_with('|') {
                if let Some(entry) = AgentInteractiveChoiceEntry::parse_table_row(line)? {
                    entries.push(entry);
                }
                continue;
            }
            if line == "details:" {
                in_details = true;
                continue;
            }
            if let Some(value) = line.strip_prefix("id:") {
                id = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("method:") {
                method = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("stage:") {
                stage = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("target:") {
                target = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("create:") {
                create = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("info:") {
                info = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("categories:") {
                categories = Some(value.trim().to_string());
            }
        }
        let choice = Self {
            id: required_interactive_field(id, "id")?,
            method: required_interactive_field(method, "method")?,
            stage: required_interactive_field(stage, "stage")?,
            target: Some(required_interactive_field(target, "target")?),
            create: required_interactive_field(create, "create")?,
            info: required_interactive_field(info, "info")?,
            categories: required_interactive_field(categories, "categories")?,
            entries,
        };
        choice.validate()?;
        Ok(choice)
    }

    fn validate(&self) -> Result<(), String> {
        if self.method != "choice" {
            return Err(format!(
                "agent-interactive `{}` must use `method: choice`, got `{}`",
                self.id, self.method
            ));
        }
        if !matches!(self.stage.as_str(), "pre-capture" | "post-materialize") {
            return Err(format!(
                "agent-interactive `{}` has unsupported `stage: {}`",
                self.id, self.stage
            ));
        }
        if self.entries.is_empty() {
            return Err(
                "agent-interactive choice details table must contain at least one row".to_string(),
            );
        }
        self.validate_categories()
    }

    fn validate_categories(&self) -> Result<(), String> {
        let mut has_detail = false;
        for part in self.categories.split(',') {
            let (key, value) = part.split_once('=').ok_or_else(|| {
                format!(
                    "agent-interactive `{}` category `{}` must use key=value",
                    self.id, part
                )
            })?;
            let key = key.trim();
            let value = value.trim();
            if key == "?" && value == "detail" {
                has_detail = true;
                continue;
            }
            if !self
                .entries
                .iter()
                .any(|entry| entry.number == key && entry.id == value)
            {
                return Err(format!(
                    "agent-interactive `{}` category `{key}={value}` must match a detail row",
                    self.id
                ));
            }
        }
        if !has_detail {
            return Err(format!(
                "agent-interactive `{}` categories must include `?=detail`",
                self.id
            ));
        }
        Ok(())
    }

    pub(in crate::command) fn render_compact(&self, contract_id: &str) -> String {
        format!(
            "{}\ninfo: {}\nload: --choice {}=?\nnext: choose --choice {}=N|ID | ask-user\nguard: resolve this interactive window before capture materializes; do not default or use --help\ncategories: {}",
            self.render_interactive_header("[agent-interactive]", Some(contract_id)),
            self.info,
            self.id,
            self.id,
            self.categories
        )
    }

    pub(in crate::command) fn render_detail(&self) -> String {
        let mut output = format!(
            "{}\nnext: choose --choice {}=N|ID | ask-user\nguard: choose only with task-specific confidence\n{}",
            self.render_interactive_header("[agent-interactive-detail]", None),
            self.id,
            "|n|id|contract|full|use-if|"
        );
        for entry in &self.entries {
            output.push('\n');
            output.push_str(&format!(
                "|{}|{}|{}|{}|{}|",
                entry.number,
                entry.id,
                entry.contract.as_deref().unwrap_or_default(),
                entry.full,
                entry.use_if
            ));
        }
        output
    }

    fn render_interactive_header(&self, label: &str, contract_id: Option<&str>) -> String {
        let contract = contract_id
            .map(|contract_id| format!(" contract={contract_id}"))
            .unwrap_or_default();
        let target = self
            .target
            .as_deref()
            .map(|target| format!(" target={target}"))
            .unwrap_or_default();
        format!(
            "{label}{contract} id={} method={} stage={}{} create={} status=interactive-required entry=not-created",
            self.id, self.method, self.stage, target, self.create
        )
    }

    pub(in crate::command) fn resolve(&self, value: &str) -> Option<&str> {
        let value = value.trim();
        if value == "?" {
            return Some("detail");
        }
        self.entries
            .iter()
            .find(|entry| entry.number == value || entry.id.eq_ignore_ascii_case(value))
            .map(|entry| entry.id.as_str())
            .or_else(|| self.resolve_category(value))
    }

    pub(in crate::command) fn contract_for(&self, selected_id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.id.eq_ignore_ascii_case(selected_id))
            .and_then(|entry| entry.contract.as_deref())
    }

    fn resolve_category(&self, value: &str) -> Option<&str> {
        self.categories.split(',').find_map(|part| {
            let (key, selected) = part.split_once('=')?;
            let key = key.trim();
            let selected = selected.trim();
            if key == value || selected.eq_ignore_ascii_case(value) {
                Some(selected)
            } else {
                None
            }
        })
    }

    pub(in crate::command) fn expected_values(&self) -> String {
        let mut values = Vec::new();
        for entry in &self.entries {
            values.push(entry.number.as_str());
            values.push(entry.id.as_str());
        }
        values.push("?");
        values.join("|")
    }
}

impl AgentInteractiveChoiceEntry {
    fn parse_table_row(line: &str) -> Result<Option<Self>, String> {
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() != 5 {
            return Err(format!(
                "agent-interactive choice detail row must have 5 cells `n|id|contract|full|use-if`: {line}"
            ));
        }
        if cells == ["n", "id", "contract", "full", "use-if"] {
            return Ok(None);
        }
        Ok(Some(Self {
            number: cells[0].to_string(),
            id: cells[1].to_string(),
            contract: optional_cell(cells[2]),
            full: cells[3].to_string(),
            use_if: cells[4].to_string(),
        }))
    }
}

fn optional_cell(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "-").then(|| value.to_string())
}

fn required_interactive_field(value: Option<String>, field: &str) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("agent-interactive choice block must declare `{field}:`"))
}

pub(in crate::command) fn choice_arg_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    for (index, arg) in args.iter().enumerate() {
        if arg == "--choice" {
            if let Some(value) = args
                .get(index + 1)
                .and_then(|value| value.strip_prefix(&prefix))
            {
                return Some(value);
            }
        } else if let Some(value) = arg.strip_prefix("--choice=")
            && let Some(value) = value.strip_prefix(&prefix)
        {
            return Some(value);
        }
    }
    None
}

pub(in crate::command) fn strip_choice_args(args: &mut Vec<String>) {
    let mut stripped = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--choice" {
            index += 2;
        } else if args[index].starts_with("--choice=") {
            index += 1;
        } else {
            stripped.push(args[index].clone());
            index += 1;
        }
    }
    *args = stripped;
}
