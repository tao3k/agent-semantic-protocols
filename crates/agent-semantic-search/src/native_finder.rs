//! Native fd/rg-backed candidate collection.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use agent_semantic_provider_transport::byte_text;
use serde_json::{Value, json};

use crate::{LanguageFileSpec, language_file_spec};

const NATIVE_CANDIDATE_LIMIT: usize = 256;
const NATIVE_PER_TERM_LIMIT: usize = 64;
const ASP_RUNTIME_BIN_DIR: &str = "ASP_RUNTIME_BIN_DIR";

/// Native finder surface to query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeFinderSurface {
    Path,
    Content,
    Both,
}

/// Candidate returned by a native finder route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFinderCandidate {
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub symbol: String,
    pub text: String,
    pub source: String,
    pub confidence: String,
}

/// Native finder collection result.
pub struct NativeFinderCandidates {
    pub candidates: Vec<NativeFinderCandidate>,
    pub provenance: NativeFinderProvenance,
}

/// Search config slice needed by native finder.
#[derive(Clone, Copy)]
pub struct NativeFinderConfig<'a> {
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
}

/// Request for native finder collection.
pub struct NativeFinderCollectionRequest<'a> {
    pub surface: NativeFinderSurface,
    pub language_id: &'a str,
    pub file_spec_override: Option<LanguageFileSpec>,
    pub accept_all_files: bool,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub roots: &'a [PathBuf],
    pub terms: &'a [String],
    pub config: NativeFinderConfig<'a>,
    pub native_args: &'a [String],
}

/// Native finder provenance fields for compact traces.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeFinderProvenance {
    backend: Option<&'static str>,
    candidate_basis: Option<&'static str>,
    source_search_passes: usize,
    file_list_passes: usize,
    input_candidates: usize,
    fallback_from: Option<&'static str>,
}

impl NativeFinderProvenance {
    #[must_use]
    pub fn input_candidate_count(&self) -> usize {
        self.input_candidates
    }

    #[must_use]
    pub fn trace_fields(&self, selected_candidates: usize) -> BTreeMap<String, Value> {
        let mut fields = BTreeMap::new();
        if let Some(backend) = self.backend {
            fields.insert("backend".to_string(), json!(backend));
        }
        if let Some(candidate_basis) = self.candidate_basis {
            fields.insert("candidateBasis".to_string(), json!(candidate_basis));
        }
        if self.source_search_passes > 0 {
            fields.insert(
                "sourceSearchPasses".to_string(),
                json!(self.source_search_passes),
            );
        }
        if self.file_list_passes > 0 {
            fields.insert("fileListPasses".to_string(), json!(self.file_list_passes));
        }
        if self.input_candidates > 0 {
            fields.insert("inputCandidates".to_string(), json!(self.input_candidates));
        }
        fields.insert("selectedCandidates".to_string(), json!(selected_candidates));
        if let Some(fallback_from) = self.fallback_from {
            fields.insert("fallbackFrom".to_string(), json!(fallback_from));
        }
        fields
    }
}

/// Collect native finder candidates using fd/rg or exa fallback.
pub fn collect_native_finder_candidates(
    request: NativeFinderCollectionRequest<'_>,
) -> Result<Option<NativeFinderCandidates>, String> {
    if request.terms.is_empty() {
        return Ok(Some(NativeFinderCandidates {
            candidates: Vec::new(),
            provenance: NativeFinderProvenance::default(),
        }));
    }
    let file_spec = request
        .file_spec_override
        .unwrap_or_else(|| language_file_spec(request.language_id));
    let mut collector = NativeFinderCollector {
        surface: request.surface,
        project_root: request.project_root,
        locator_root: request.locator_root,
        roots: request.roots,
        terms: request.terms,
        normalized_terms: request
            .terms
            .iter()
            .map(|term| term.to_ascii_lowercase())
            .collect(),
        file_spec,
        accept_all_files: request.accept_all_files,
        config: request.config,
        native_args: request.native_args,
        seen: HashSet::new(),
        candidates: Vec::new(),
        provenance: NativeFinderProvenance::default(),
    };
    let ran_any = match request.surface {
        NativeFinderSurface::Path => collector.append_fd_candidates()?,
        NativeFinderSurface::Content => collector.append_rg_candidates()?,
        NativeFinderSurface::Both => {
            let rg_ran = collector.append_rg_candidates()?;
            if !collector.candidates.is_empty() || collector.is_done() {
                collector.sort_path_candidates();
                return Ok(Some(NativeFinderCandidates {
                    candidates: collector.candidates,
                    provenance: collector.provenance,
                }));
            }
            let fd_ran = collector.append_fd_candidates()?;
            fd_ran || rg_ran
        }
    };
    if ran_any {
        collector.sort_path_candidates();
        Ok(Some(NativeFinderCandidates {
            candidates: collector.candidates,
            provenance: collector.provenance,
        }))
    } else {
        Ok(None)
    }
}

struct NativeFinderCollector<'a> {
    surface: NativeFinderSurface,
    project_root: &'a Path,
    locator_root: &'a Path,
    roots: &'a [PathBuf],
    terms: &'a [String],
    normalized_terms: Vec<String>,
    file_spec: LanguageFileSpec,
    accept_all_files: bool,
    config: NativeFinderConfig<'a>,
    native_args: &'a [String],
    seen: HashSet<String>,
    candidates: Vec<NativeFinderCandidate>,
    provenance: NativeFinderProvenance,
}

impl NativeFinderCollector<'_> {
    fn append_fd_candidates(&mut self) -> Result<bool, String> {
        if native_command("fd").is_none() {
            return self.append_exa_candidates();
        }
        let mut ran_any = false;
        for request in self.search_requests() {
            if self.is_done() {
                return Ok(ran_any);
            }
            ran_any |= self.append_fd_request(request)?;
        }
        Ok(ran_any)
    }

    fn append_exa_candidates(&mut self) -> Result<bool, String> {
        let mut ran_any = false;
        for root in self.unique_roots() {
            if self.is_done() {
                return Ok(ran_any);
            }
            ran_any |= self.append_exa_root(&root)?;
        }
        Ok(ran_any)
    }

    fn append_rg_candidates(&mut self) -> Result<bool, String> {
        let mut ran_any = false;
        let requests = if self.surface == NativeFinderSurface::Content {
            self.search_requests()
        } else {
            self.content_search_requests()
        };
        for request in requests {
            if self.is_done() {
                return Ok(ran_any);
            }
            ran_any |= self.append_rg_request(request)?;
        }
        Ok(ran_any)
    }

    fn search_requests(&self) -> Vec<NativeFinderRequest> {
        let Some(pattern) = native_search_pattern(self.terms) else {
            return Vec::new();
        };
        self.roots
            .iter()
            .map(|root| NativeFinderRequest {
                root: root.clone(),
                pattern: pattern.clone(),
            })
            .collect()
    }

    fn content_search_requests(&self) -> Vec<NativeFinderRequest> {
        native_search_patterns(self.terms)
            .into_iter()
            .flat_map(|pattern| {
                self.roots.iter().map(move |root| NativeFinderRequest {
                    root: root.clone(),
                    pattern: pattern.clone(),
                })
            })
            .collect()
    }

    fn unique_roots(&self) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        self.roots
            .iter()
            .filter(|root| seen.insert((*root).clone()))
            .cloned()
            .collect()
    }

    fn append_fd_request(&mut self, request: NativeFinderRequest) -> Result<bool, String> {
        let Some(stdout) = self.run_fd(&request.pattern, &request.root)? else {
            return Ok(false);
        };
        self.provenance.backend = Some("fd");
        self.provenance.candidate_basis = Some("paths");
        self.provenance.source_search_passes += 1;
        self.provenance.file_list_passes += 1;
        for line in byte_text::split_lf_lines(stdout.as_slice()) {
            if self.is_done() {
                break;
            }
            if line.is_empty() {
                continue;
            }
            self.provenance.input_candidates += 1;
            if let Some(candidate) = self.path_candidate(line) {
                self.push(candidate);
            }
        }
        Ok(true)
    }

    fn append_exa_root(&mut self, root: &Path) -> Result<bool, String> {
        let Some(stdout) = self.run_exa(root)? else {
            return Ok(false);
        };
        self.provenance.backend = Some("fd+exa");
        self.provenance.candidate_basis = Some("paths");
        self.provenance.source_search_passes += 1;
        self.provenance.file_list_passes += 1;
        self.provenance.fallback_from = Some("fd");
        for line in byte_text::split_lf_lines(stdout.as_slice()) {
            if self.is_done() {
                break;
            }
            if line.is_empty() {
                continue;
            }
            self.provenance.input_candidates += 1;
            let raw = byte_text::lossy_string(line);
            if let Some(term_index) = self.matching_path_term_index(&raw) {
                let term = self.terms[term_index].clone();
                if let Some(candidate) = self.path_candidate_from_raw(&raw, &term) {
                    self.push(candidate);
                }
            }
        }
        Ok(true)
    }

    fn append_rg_request(&mut self, request: NativeFinderRequest) -> Result<bool, String> {
        let Some(mut command) = self.rg_command(&request.pattern, &request.root) else {
            return Ok(false);
        };
        command.stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(format!("failed to run native rg: {error}")),
        };
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to capture native rg stdout".to_string())?;
        self.provenance.backend = Some("rg");
        self.provenance.candidate_basis = Some("source-lines");
        self.provenance.source_search_passes += 1;
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        while !self.is_done() {
            line.clear();
            let read = reader
                .read_until(b'\n', &mut line)
                .map_err(|error| format!("failed to read native rg stdout: {error}"))?;
            if read == 0 {
                break;
            }
            trim_line_end(&mut line);
            if self.is_done() || line.is_empty() {
                continue;
            }
            self.provenance.input_candidates += 1;
            if let Some(candidate) = self.rg_candidate(&line) {
                self.push(candidate);
            }
        }
        if self.is_done() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(true);
        }
        let status = child
            .wait()
            .map_err(|error| format!("failed to wait for native rg: {error}"))?;
        if !(status.success() || status.code() == Some(1)) {
            return Ok(false);
        }
        Ok(true)
    }

    fn run_fd(&self, pattern: &str, root: &Path) -> Result<Option<Vec<u8>>, String> {
        let Some(mut command) = native_command("fd") else {
            return Ok(None);
        };
        command
            .arg("--type")
            .arg("f")
            .arg("--color")
            .arg("never")
            .arg("--ignore-case")
            .args(self.native_args)
            .arg(pattern)
            .arg(root);
        if !self.accept_all_files && !root.is_file() && self.file_spec.config_filenames().is_empty()
        {
            for extension in self.file_spec.extensions() {
                command.arg("--extension").arg(extension);
            }
        }
        append_fd_ignores(&mut command, self.config);
        native_stdout(command, "fd")
    }

    fn run_exa(&self, root: &Path) -> Result<Option<Vec<u8>>, String> {
        let Some(mut command) = native_command("exa") else {
            return Ok(None);
        };
        command
            .arg("--recurse")
            .arg("--only-files")
            .arg("--oneline")
            .arg("--color=never")
            .arg(root);
        native_stdout(command, "exa")
    }

    fn rg_command(&self, pattern: &str, root: &Path) -> Option<Command> {
        let mut command = native_command("rg")?;
        command
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--with-filename")
            .arg("--color")
            .arg("never")
            .arg("--max-count")
            .arg(NATIVE_PER_TERM_LIMIT.to_string())
            .arg("--ignore-case")
            .args(self.native_args)
            .arg(pattern);
        if !self.accept_all_files && !root.is_file() {
            for extension in self.file_spec.extensions() {
                command.arg("--glob").arg(format!("*.{extension}"));
            }
            for config_file in self.file_spec.config_filenames() {
                command.arg("--glob").arg(format!("**/{config_file}"));
            }
        }
        for dir in self.config.ignore_dirs {
            command.arg("--glob").arg(format!("!{dir}/**"));
        }
        command.arg(root);
        Some(command)
    }

    fn path_candidate(&self, line: &[u8]) -> Option<NativeFinderCandidate> {
        let raw = byte_text::lossy_string(line);
        let term_index = self.matching_path_term_index(&raw)?;
        self.path_candidate_from_raw(&raw, &self.terms[term_index])
    }

    fn path_candidate_from_raw(&self, raw: &str, term: &str) -> Option<NativeFinderCandidate> {
        let path = resolve_native_path(self.project_root, raw);
        if !self.accepts_candidate_path(&path) {
            return None;
        }
        let display = display_path(self.locator_root, &path);
        Some(NativeFinderCandidate {
            path: display.clone(),
            line: 1,
            end_line: 1,
            symbol: term.to_string(),
            text: display,
            source: self.path_source().to_string(),
            confidence: "likely".to_string(),
        })
    }

    fn matching_path_term_index(&self, raw: &str) -> Option<usize> {
        let normalized_path = raw.to_ascii_lowercase();
        self.normalized_terms
            .iter()
            .position(|term| !term.is_empty() && normalized_path.contains(term))
    }

    fn rg_candidate(&self, line: &[u8]) -> Option<NativeFinderCandidate> {
        let (path, line_number, text) = parse_rg_line(line)?;
        let path = resolve_native_path(self.project_root, &path);
        if !self.accepts_candidate_path(&path) {
            return None;
        }
        let term = self
            .matching_content_term_index(&text)
            .and_then(|index| self.terms.get(index))
            .or_else(|| self.terms.first())?;
        Some(NativeFinderCandidate {
            path: display_path(self.locator_root, &path),
            line: line_number,
            end_line: line_number,
            symbol: term.to_string(),
            text,
            source: self.content_source().to_string(),
            confidence: "heuristic".to_string(),
        })
    }

    fn matching_content_term_index(&self, text: &str) -> Option<usize> {
        let normalized_text = text.to_ascii_lowercase();
        self.normalized_terms
            .iter()
            .position(|term| !term.is_empty() && normalized_text.contains(term))
    }

    fn accepts_candidate_path(&self, path: &Path) -> bool {
        path.is_file()
            && (self.accept_all_files
                || self.file_spec.matches(path)
                || self.is_explicit_file_scope(path))
            && !ignored_by_config(path, self.project_root, self.config)
    }

    fn is_explicit_file_scope(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| {
            root.is_file() && (root == path || paths_resolve_to_same_file(root.as_path(), path))
        })
    }

    fn push(&mut self, candidate: NativeFinderCandidate) {
        let key = format!(
            "{}:{}:{}:{}",
            candidate.path, candidate.line, candidate.symbol, candidate.text
        );
        if self.seen.insert(key) {
            self.candidates.push(candidate);
        }
    }

    fn is_done(&self) -> bool {
        self.candidates.len() >= NATIVE_CANDIDATE_LIMIT
    }

    fn sort_path_candidates(&mut self) {
        if self.surface != NativeFinderSurface::Path {
            return;
        }
        self.candidates.sort_by_key(|candidate| {
            native_path_candidate_sort_key(candidate, self.normalized_terms.as_slice())
        });
    }

    fn path_source(&self) -> &'static str {
        match self.surface {
            NativeFinderSurface::Path => "fd-query",
            NativeFinderSurface::Content | NativeFinderSurface::Both => "finder-path",
        }
    }

    fn content_source(&self) -> &'static str {
        match self.surface {
            NativeFinderSurface::Path => "finder",
            NativeFinderSurface::Content => "rg-query",
            NativeFinderSurface::Both => "finder",
        }
    }
}

type NativePathCandidateSortKey = (Reverse<usize>, String);

fn native_path_candidate_sort_key(
    candidate: &NativeFinderCandidate,
    normalized_terms: &[String],
) -> NativePathCandidateSortKey {
    let normalized_path = candidate.path.to_ascii_lowercase();
    (
        Reverse(
            normalized_terms
                .iter()
                .filter(|term| !term.is_empty() && normalized_path.contains(term.as_str()))
                .count(),
        ),
        candidate.path.clone(),
    )
}

fn paths_resolve_to_same_file(left: &Path, right: &Path) -> bool {
    let Ok(left) = left.canonicalize() else {
        return false;
    };
    let Ok(right) = right.canonicalize() else {
        return false;
    };
    left == right
}

fn native_command(label: &str) -> Option<Command> {
    native_command_path(label).map(Command::new)
}

fn native_command_path(label: &str) -> Option<PathBuf> {
    env::var_os(ASP_RUNTIME_BIN_DIR)
        .map(PathBuf::from)
        .map(|runtime_bin| runtime_bin.join(label))
        .filter(|candidate| candidate.is_file())
        .or_else(|| which::which(label).ok())
}

struct NativeFinderRequest {
    root: PathBuf,
    pattern: String,
}

fn native_search_pattern(terms: &[String]) -> Option<String> {
    let escaped = native_search_patterns(terms);
    (!escaped.is_empty()).then(|| escaped.join("|"))
}

fn native_search_patterns(terms: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    terms
        .iter()
        .filter_map(|term| {
            let term = term.trim();
            (!term.is_empty() && seen.insert(term.to_ascii_lowercase()))
                .then(|| escape_native_regex(term))
        })
        .collect()
}

fn escape_native_regex(term: &str) -> String {
    let mut escaped = String::with_capacity(term.len());
    for character in term.chars() {
        if matches!(
            character,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn native_stdout(mut command: Command, label: &str) -> Result<Option<Vec<u8>>, String> {
    let output = match command.output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to run native {label}: {error}")),
    };
    if output.status.success() || output.status.code() == Some(1) {
        return Ok(Some(output.stdout));
    }
    Ok(None)
}

fn append_fd_ignores(command: &mut Command, config: NativeFinderConfig<'_>) {
    for dir in config.ignore_dirs {
        command.arg("--exclude").arg(dir);
    }
    if !config.include_hidden_dirs.is_empty() {
        command.arg("--hidden");
    }
}

fn trim_line_end(line: &mut Vec<u8>) {
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
}

fn parse_rg_line(line: &[u8]) -> Option<(String, usize, String)> {
    let path_end = byte_text::find_byte(b':', line)?;
    let path = byte_text::lossy_string(&line[..path_end]);
    let rest = &line[path_end + 1..];
    let line_end = byte_text::find_byte(b':', rest)?;
    let line_number = std::str::from_utf8(&rest[..line_end])
        .ok()?
        .parse::<usize>()
        .ok()?;
    let text = byte_text::lossy_string(&rest[line_end + 1..]);
    Some((path, line_number, text))
}

fn resolve_native_path(project_root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    let cwd_relative = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(&path))
        .filter(|candidate| candidate.exists());
    cwd_relative.unwrap_or_else(|| project_root.join(path))
}

fn ignored_by_config(path: &Path, project_root: &Path, config: NativeFinderConfig<'_>) -> bool {
    let relative = path.strip_prefix(project_root).unwrap_or(path);
    relative.components().any(|component| {
        let label = component.as_os_str().to_string_lossy();
        config.ignore_dirs.iter().any(|dir| dir == &label)
    })
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
