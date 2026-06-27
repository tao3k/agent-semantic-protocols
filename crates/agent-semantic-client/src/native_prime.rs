//! Native client-side `search prime --view seeds` frontier.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use agent_semantic_client_core::{ClientMethod, ClientRequest, LanguageId};
use bytes::Bytes;

const NATIVE_PRIME_OWNER_LIMIT: usize = 12;
const NATIVE_PRIME_SCAN_DIR_LIMIT: usize = 128;
const NATIVE_PRIME_SCAN_FILE_LIMIT: usize = 512;
const NATIVE_PRIME_SKIP_DIRS: &[&str] = &[
    ".cache",
    ".codex",
    ".devenv",
    ".direnv",
    ".git",
    ".hg",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
    "venv",
];

pub(crate) fn render_native_prime_seed_stdout(
    project_root: &Path,
    request: &ClientRequest,
    receipt_json: bool,
) -> Result<Option<Bytes>, String> {
    if receipt_json || !is_prime_seed_search(request) {
        return Ok(None);
    }
    let Some(language_id) = request.language_id.as_ref() else {
        return Ok(None);
    };
    let Some(file_spec) = NativePrimeFileSpec::for_language(language_id) else {
        return Ok(None);
    };
    let owners = native_prime_owners(project_root, file_spec)?;
    Ok(Some(Bytes::from(render_prime_seed_text(
        project_root,
        &owners,
    ))))
}

fn is_prime_seed_search(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Search
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "prime")
        && has_seed_view(&request.forwarded_args)
        && !request
            .forwarded_args
            .iter()
            .any(|arg| arg == "--json" || arg == "--code" || arg == "items" || arg == "ingest")
}

fn has_seed_view(args: &[String]) -> bool {
    args.windows(2)
        .any(|window| window[0] == "--view" && window[1] == "seeds")
        || args.iter().any(|arg| arg == "--view=seeds")
}

fn native_prime_owners(
    project_root: &Path,
    file_spec: NativePrimeFileSpec,
) -> Result<Vec<String>, String> {
    if let Some(owners) = fd_prime_owners(project_root, file_spec)? {
        return Ok(owners);
    }
    fs_prime_owners(project_root, file_spec)
}

fn fd_prime_owners(
    project_root: &Path,
    file_spec: NativePrimeFileSpec,
) -> Result<Option<Vec<String>>, String> {
    let mut command = Command::new("fd");
    command
        .arg("--type")
        .arg("f")
        .arg("--hidden")
        .arg("--color")
        .arg("never")
        .arg(".")
        .arg(project_root);
    for extension in file_spec.extensions {
        command.arg("--extension").arg(extension);
    }
    for dir in NATIVE_PRIME_SKIP_DIRS {
        command.arg("--exclude").arg(dir);
    }
    let output = match command.output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to run native prime fd: {error}")),
    };
    if !(output.status.success() || output.status.code() == Some(1)) {
        return Ok(None);
    }
    let owners = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| relative_owner_path(project_root, Path::new(line)))
        .take(NATIVE_PRIME_OWNER_LIMIT)
        .collect::<Vec<_>>();
    if owners.is_empty() {
        Ok(None)
    } else {
        Ok(Some(owners))
    }
}

fn fs_prime_owners(
    project_root: &Path,
    file_spec: NativePrimeFileSpec,
) -> Result<Vec<String>, String> {
    let mut traversal = FsPrimeTraversal::new(project_root, file_spec);
    while let Some(dir) = traversal.next_dir() {
        if traversal.is_done() {
            break;
        }
        traversal.scan_dir(&dir);
    }
    Ok(traversal.owners)
}

struct FsPrimeTraversal<'a> {
    project_root: &'a Path,
    file_spec: NativePrimeFileSpec,
    dirs: Vec<std::path::PathBuf>,
    seen_dirs: BTreeSet<std::path::PathBuf>,
    owners: Vec<String>,
    scanned_files: usize,
}

impl<'a> FsPrimeTraversal<'a> {
    fn new(project_root: &'a Path, file_spec: NativePrimeFileSpec) -> Self {
        Self {
            project_root,
            file_spec,
            dirs: vec![project_root.to_path_buf()],
            seen_dirs: BTreeSet::new(),
            owners: Vec::new(),
            scanned_files: 0,
        }
    }

    fn next_dir(&mut self) -> Option<std::path::PathBuf> {
        self.dirs.pop()
    }

    fn is_done(&self) -> bool {
        self.owners.len() >= NATIVE_PRIME_OWNER_LIMIT
            || self.seen_dirs.len() >= NATIVE_PRIME_SCAN_DIR_LIMIT
            || self.scanned_files > NATIVE_PRIME_SCAN_FILE_LIMIT
    }

    fn scan_dir(&mut self, dir: &Path) {
        let dir_key = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        if !self.seen_dirs.insert(dir_key) || should_skip_dir(dir) {
            return;
        }
        for entry in sorted_read_dir_entries(dir) {
            self.scan_entry(entry);
            if self.is_done() {
                return;
            }
        }
    }

    fn scan_entry(&mut self, entry: fs::DirEntry) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            return;
        };
        if file_type.is_dir() {
            self.dirs.push(path);
            return;
        }
        if file_type.is_file() && self.file_spec.matches(&path) {
            self.scanned_files += 1;
            if let Some(owner) = relative_owner_path(self.project_root, &path) {
                self.owners.push(owner);
            }
        }
    }
}

fn sorted_read_dir_entries(dir: &Path) -> Vec<fs::DirEntry> {
    let mut entries = match fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };
    entries.sort_by_key(|entry| entry.path());
    entries
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| NATIVE_PRIME_SKIP_DIRS.contains(&name))
}

fn relative_owner_path(project_root: &Path, path: &Path) -> Option<String> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    path.strip_prefix(project_root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
}

fn render_prime_seed_text(project_root: &Path, owners: &[String]) -> String {
    let root_label = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".");
    let mut lines = vec![
        format!(
            "[search-prime] root={root_label} alg=native-fd-prime-frontier-v1 budget=handles:{NATIVE_PRIME_OWNER_LIMIT}"
        ),
        format!(
            "|decision purpose=decision-primer answer=false code=false route=evidence-state capabilities=pipe,fzf,fd-query,rg-query,owner-items,selector-code,treesitter-query history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath risk=broad-direct-read,manual-window-scan,repeat-prime rule=\"prime maps workspace/owners only; choose the narrowest route justified by current evidence\" routeOptions=\"owner-items when owner known; selector-code when exact selector known; deps when dependency known; pipe/fzf only for ambiguous query refinement\""
        ),
        "[route-graph] profile=asp-search-routing evidence=unknown-workspace chosen=UNKNOWN_WORKSPACE reason=\"no owner/symbol/selector evidence supplied; prime maps workspace owners only\" frontier=A1.owner-map".to_string(),
        "A1=route-action(route=UNKNOWN_WORKSPACE,targetRole=path,projection=topology,codePolicy=disabled,avoid=direct-source-read|line-range-selector)!owner-map".to_string(),
        "actionFrontier=A1.owner-map".to_string(),
        "recommendedNext=A1.owner-map".to_string(),
        "legend: ID=kind:role(value)!next; entries profile(selectors=>returns); frontier ID.next"
            .to_string(),
        "aliases: owner:{O=owner}".to_string(),
    ];
    let owner_ids = owners
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if index == 0 {
                "O".to_string()
            } else {
                format!("O{}", index + 1)
            }
        })
        .collect::<Vec<_>>();
    if !owners.is_empty() {
        lines.push(
            owners
                .iter()
                .zip(owner_ids.iter())
                .map(|(owner, owner_id)| format!("{owner_id}=owner:path({owner})!owner"))
                .collect::<Vec<_>>()
                .join(";"),
        );
    }
    lines.push("entries=owner-tests(O=>covering-tests+test-entrypoints+fixtures)".to_string());
    lines.push("omit=items,blocks,code,full-test-list".to_string());
    lines.push("avoid=raw-read,full-json,broad-fzf".to_string());
    lines.push(String::new());
    lines.join("\n")
}

#[derive(Clone, Copy)]
struct NativePrimeFileSpec {
    extensions: &'static [&'static str],
}

impl NativePrimeFileSpec {
    fn for_language(language_id: &LanguageId) -> Option<Self> {
        let extensions = match language_id.as_str() {
            "rust" => &["rs"][..],
            "typescript" => &["ts", "tsx", "js", "jsx"][..],
            "python" => &["py"][..],
            "julia" => &["jl"][..],
            "gerbil-scheme" => &["ss", "ssi", "scm", "sld"][..],
            "org" => &["org"][..],
            "md" => &["md"][..],
            _ => return None,
        };
        Some(Self { extensions })
    }

    fn matches(self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| self.extensions.contains(&extension))
    }
}
