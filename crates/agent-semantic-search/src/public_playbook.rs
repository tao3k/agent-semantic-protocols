//! Strict public `search playbook` argument contract.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlaybookRequest {
    pub query: String,
    pub intent: String,
    pub scope: String,
    pub coverage: String,
    pub max_owners: u32,
    pub deadline_ms: u64,
    pub explain: String,
    pub language: Option<String>,
    pub workspace: String,
}

pub fn parse_search_playbook_args(args: &[String]) -> Result<SearchPlaybookRequest, String> {
    if args.first().map(String::as_str) != Some("search")
        || args.get(1).map(String::as_str) != Some("playbook")
    {
        let operation = args.get(1).map(String::as_str).unwrap_or("<missing>");
        return Err(format!(
            "search operation `{operation}` was removed; use search playbook <query>"
        ));
    }
    let mut request = SearchPlaybookRequest {
        query: String::new(),
        intent: "conceptual".to_owned(),
        scope: "workspace".to_owned(),
        coverage: "candidates".to_owned(),
        max_owners: 100,
        deadline_ms: 1_000,
        explain: "compact".to_owned(),
        language: None,
        workspace: ".".to_owned(),
    };
    let mut query_terms = Vec::new();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--intent" => request.intent = value(args, &mut index, "--intent")?,
            "--scope" => request.scope = value(args, &mut index, "--scope")?,
            "--coverage" => request.coverage = value(args, &mut index, "--coverage")?,
            "--max-owners" => {
                request.max_owners = value(args, &mut index, "--max-owners")?
                    .parse()
                    .map_err(|_| "--max-owners requires a positive integer".to_owned())?;
            }
            "--deadline-ms" => {
                request.deadline_ms = value(args, &mut index, "--deadline-ms")?
                    .parse()
                    .map_err(|_| "--deadline-ms requires a positive integer".to_owned())?;
            }
            "--explain" => request.explain = value(args, &mut index, "--explain")?,
            "--language" | "-l" => {
                request.language = Some(value(args, &mut index, "--language")?);
            }
            "--query" => query_terms.push(value(args, &mut index, "--query")?),
            "--workspace" => request.workspace = value(args, &mut index, "--workspace")?,
            "--view" | "--seeds" | "--query-set" | "--owner" | "--from-hook" | "--projection"
            | "--context" => {
                return Err(format!(
                    "search playbook removed legacy option `{}`",
                    args[index]
                ));
            }
            option if option.starts_with('-') => {
                return Err(format!(
                    "search playbook does not support option `{option}`"
                ));
            }
            term => {
                query_terms.push(term.to_owned());
                index += 1;
            }
        }
    }
    request.query = query_terms.join(" ");
    request.validate()?;
    Ok(request)
}

impl SearchPlaybookRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.query.trim().is_empty() {
            return Err("search playbook requires a non-empty query".to_owned());
        }
        super::ResidentSearchIntent::parse(&self.intent)?;
        if self.scope != "workspace"
            && !self
                .scope
                .strip_prefix("owner:")
                .is_some_and(|owner| !owner.trim().is_empty())
        {
            return Err("search playbook scope must be workspace or owner:<path>".to_owned());
        }
        if !matches!(self.coverage.as_str(), "candidates" | "complete") {
            return Err("search playbook coverage must be candidates or complete".to_owned());
        }
        if self.coverage == "complete" && self.intent != "absence-proof" {
            return Err("complete coverage requires absence-proof intent".to_owned());
        }
        if self.max_owners == 0 || self.max_owners > 100 {
            return Err("search playbook maxOwners must be between 1 and 100".to_owned());
        }
        if self.deadline_ms == 0 || self.deadline_ms > 5_000 {
            return Err("search playbook deadlineMs must be between 1 and 5000".to_owned());
        }
        if !matches!(self.explain.as_str(), "compact" | "full") {
            return Err("search playbook explain must be compact or full".to_owned());
        }
        if self
            .language
            .as_ref()
            .is_some_and(|language| language.trim().is_empty())
        {
            return Err("search playbook language must not be empty".to_owned());
        }
        if self.workspace.trim().is_empty() {
            return Err("search playbook workspace must not be empty".to_owned());
        }
        Ok(())
    }
}

fn value(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    let value = args
        .get(*index + 1)
        .filter(|value| !value.starts_with('-'))
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))?;
    *index += 2;
    Ok(value)
}
