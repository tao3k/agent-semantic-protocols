//! Document-language candidate collection for search pipe acquisition.

use std::path::{Path, PathBuf};

use orgize::document::{
    DocumentElement, DocumentLanguage, DocumentWalkConfig, filter_elements,
    index_project_with_config,
};

const DOCUMENT_PIPE_CANDIDATE_LIMIT: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSearchCandidate {
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub symbol: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSearchCandidateCollection {
    pub candidates: Vec<DocumentSearchCandidate>,
    pub matched_count: usize,
}

pub struct DocumentSearchCandidateRequest<'a> {
    pub language: DocumentLanguage,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub intent: &'a str,
    pub scopes: &'a [PathBuf],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
}

pub fn collect_document_search_candidates(
    request: DocumentSearchCandidateRequest<'_>,
) -> Result<DocumentSearchCandidateCollection, String> {
    let walk_config = DocumentWalkConfig::new(
        request.ignore_dirs.to_vec(),
        request.include_hidden_dirs.to_vec(),
    );
    let mut elements = Vec::new();
    for root in document_search_roots(request.project_root, request.scopes) {
        elements.extend(index_project_with_config(
            request.language,
            &root,
            &walk_config,
        )?);
    }
    let matches = filter_elements(&elements, request.intent);
    let candidates = matches
        .iter()
        .take(DOCUMENT_PIPE_CANDIDATE_LIMIT)
        .map(|element| document_candidate(element, request.locator_root))
        .collect::<Vec<_>>();
    Ok(DocumentSearchCandidateCollection {
        candidates,
        matched_count: matches.len(),
    })
}

fn document_candidate(element: &DocumentElement, locator_root: &Path) -> DocumentSearchCandidate {
    DocumentSearchCandidate {
        path: display_document_path(locator_root, &element.path),
        line: element.line,
        end_line: element.end_line.max(element.line),
        symbol: document_symbol(element),
        text: document_candidate_text(element),
    }
}

fn document_symbol(element: &DocumentElement) -> String {
    element
        .fields
        .iter()
        .find(|(key, value)| {
            matches!(
                key.as_str(),
                "title" | "key" | "value" | "lang" | "target" | "description"
            ) && !value.trim().is_empty()
        })
        .map(|(_, value)| symbol_from_text(value))
        .filter(|symbol| !symbol.is_empty())
        .unwrap_or_else(|| element.kind.to_string())
}

fn document_candidate_text(element: &DocumentElement) -> String {
    let mut text = format!("{} {}", element.kind, element.source_kind);
    for (key, value) in &element.fields {
        if !value.trim().is_empty() {
            text.push(' ');
            text.push_str(key);
            text.push('=');
            text.push_str(value);
        }
    }
    if !element.text.trim().is_empty() {
        text.push(' ');
        text.push_str(element.text.trim());
    } else if !element.content.trim().is_empty() {
        text.push(' ');
        text.push_str(element.content.trim());
    }
    text
}

fn display_document_path(locator_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    path.strip_prefix(locator_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn symbol_from_text(text: &str) -> String {
    text.split(|character: char| {
        !(character == '_' || character == '-' || character.is_ascii_alphanumeric())
    })
    .find(|part| !part.is_empty())
    .unwrap_or("match")
    .to_lowercase()
}

fn document_search_roots(project_root: &Path, scopes: &[PathBuf]) -> Vec<PathBuf> {
    if scopes.is_empty() {
        return vec![project_root.to_path_buf()];
    }
    scopes
        .iter()
        .map(|scope| {
            if scope.is_absolute() {
                scope.clone()
            } else {
                project_root.join(scope)
            }
        })
        .collect()
}
