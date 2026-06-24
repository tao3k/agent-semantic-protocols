use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Serialize)]
pub(super) struct OrgPlanCandidate {
    pub(super) path: PathBuf,
    pub(super) title: String,
    pub(super) todo: String,
    pub(super) todo_type: String,
    pub(super) properties: BTreeMap<String, String>,
    pub(super) mtime: f64,
}

pub(super) struct RankedOrgPlan {
    pub(super) candidate: OrgPlanCandidate,
    pub(super) score: f64,
    pub(super) text_score: f64,
    pub(super) memory_score: f64,
    pub(super) recency_score: f64,
    pub(super) intent_score: f64,
}

impl OrgPlanCandidate {
    pub(super) fn plan_id(&self) -> String {
        self.properties
            .get("PLAN_ID")
            .or_else(|| self.properties.get("ID"))
            .cloned()
            .unwrap_or_else(|| {
                self.path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("agent-plan")
                    .to_string()
            })
    }

    pub(super) fn objective(&self) -> String {
        self.properties
            .get("OBJECTIVE")
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn next_action(&self) -> String {
        self.properties
            .get("NEXT_ACTION")
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn recovery_ref(&self) -> String {
        self.properties
            .get("RECOVERY_REF")
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn display_title(&self) -> String {
        self.title
            .split_whitespace()
            .filter(|token| !is_progress_cookie(token))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn is_progress_cookie(token: &str) -> bool {
    let Some(inner) = token
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };
    if let Some(percent) = inner.strip_suffix('%') {
        return percent.chars().all(|ch| ch.is_ascii_digit());
    }
    let Some((left, right)) = inner.split_once('/') else {
        return false;
    };
    left.chars().all(|ch| ch.is_ascii_digit()) && right.chars().all(|ch| ch.is_ascii_digit())
}
