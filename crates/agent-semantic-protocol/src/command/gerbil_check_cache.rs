//! Rust-side replay for the Gerbil full-check output cache.

use std::fs;
use std::io::Read;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const CHECK_CACHE_VERSION: &str = "check-full-output-cache.v1";
const FNV64_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV64_PRIME: u64 = 1_099_511_628_211;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Sexp {
    List(Vec<Sexp>),
    Pair(Box<Sexp>, Box<Sexp>),
    Symbol(String),
    String(String),
    Integer(i64),
}

#[derive(Debug)]
struct CheckCache {
    version: String,
    fingerprint: String,
    inputs: Vec<String>,
    directories: Vec<String>,
    output: String,
}

pub(super) fn try_replay_gerbil_check_cache(
    language_id: &str,
    args: &[String],
    project_root: &Path,
) -> Result<bool, String> {
    if language_id != "gerbil-scheme" || !check_cache_eligible(args) {
        debug_miss("ineligible", None);
        return Ok(false);
    }
    let mode = if has_flag(args, "--json") {
        "json"
    } else {
        "text"
    };
    let cache_path = project_root
        .join(".cache/agent-semantic-protocol/gerbil-scheme/check")
        .join(format!("{mode}.sexp"));
    let Some(cache) = read_check_cache(&cache_path) else {
        debug_miss("read-cache", Some(&cache_path));
        return Ok(false);
    };
    if cache.version != CHECK_CACHE_VERSION {
        debug_miss("version", Some(&cache_path));
        return Ok(false);
    }
    let fingerprint = check_cache_fingerprint(project_root, &cache.inputs, &cache.directories);
    if fingerprint != cache.fingerprint {
        debug_miss("fingerprint", Some(&cache_path));
        return Ok(false);
    }
    io::stdout()
        .write_all(cache.output.as_bytes())
        .map_err(|error| format!("failed to write Gerbil check cache replay: {error}"))?;
    Ok(true)
}

fn debug_miss(reason: &str, path: Option<&Path>) {
    if std::env::var_os("ASP_DEBUG_GERBIL_CHECK_CACHE").is_none() {
        return;
    }
    match path {
        Some(path) => eprintln!(
            "[asp-gerbil-check-cache] miss reason={reason} path={}",
            path.display()
        ),
        None => eprintln!("[asp-gerbil-check-cache] miss reason={reason}"),
    }
}

fn check_cache_eligible(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "check")
        && !has_flag(args, "--profile-json")
        && !has_flag(args, "--changed")
        && !has_flag(args, "changed")
        && !has_flag(args, "--receipt-json")
        && !has_flag(args, "--view")
        && !has_option(args, "--whitelist")
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn has_option(args: &[String], option: &str) -> bool {
    args.iter()
        .any(|arg| arg == option || arg.starts_with(&format!("{option}=")))
}

fn read_check_cache(path: &Path) -> Option<CheckCache> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            debug_miss_detail("read-cache", path, &error.to_string());
            return None;
        }
    };
    let sexp = match Parser::new(&source).parse() {
        Ok(sexp) => sexp,
        Err(error) => {
            debug_miss_detail("parse-cache", path, &error);
            return None;
        }
    };
    debug_cache_shape(path, &sexp);
    let cache = CheckCache::from_sexp(sexp);
    if cache.is_none() {
        debug_miss_detail(
            "shape-cache",
            path,
            "missing version/fingerprint/inputs/directories/output",
        );
    }
    cache
}

fn debug_cache_shape(path: &Path, sexp: &Sexp) {
    if std::env::var_os("ASP_DEBUG_GERBIL_CHECK_CACHE").is_none() {
        return;
    }
    for key in ["version", "fingerprint", "inputs", "directories", "output"] {
        let shape = alist_value_shape(sexp, key).unwrap_or("missing");
        eprintln!(
            "[asp-gerbil-check-cache] shape key={key} shape={shape} path={}",
            path.display()
        );
    }
}

fn alist_value_shape(sexp: &Sexp, key: &str) -> Option<&'static str> {
    if let Some(value) = alist_value(sexp, key) {
        return Some(sexp_shape(value));
    }
    let Sexp::List(entries) = sexp else {
        return None;
    };
    entries.iter().find_map(|entry| {
        let Sexp::List(items) = entry else {
            return None;
        };
        if items.first().is_some_and(|head| matches_symbol(head, key)) {
            Some("proper-list")
        } else {
            None
        }
    })
}

fn sexp_shape(sexp: &Sexp) -> &'static str {
    match sexp {
        Sexp::List(_) => "list",
        Sexp::Pair(_, _) => "pair",
        Sexp::Symbol(_) => "symbol",
        Sexp::String(_) => "string",
        Sexp::Integer(_) => "integer",
    }
}

fn debug_miss_detail(reason: &str, path: &Path, detail: &str) {
    if std::env::var_os("ASP_DEBUG_GERBIL_CHECK_CACHE").is_none() {
        return;
    }
    eprintln!(
        "[asp-gerbil-check-cache] miss reason={reason} path={} detail={detail}",
        path.display()
    );
}

impl CheckCache {
    fn from_sexp(sexp: Sexp) -> Option<Self> {
        let version = alist_string(&sexp, "version")?;
        let fingerprint = alist_string(&sexp, "fingerprint")?;
        let inputs = alist_string_list(&sexp, "inputs")?;
        let directories = alist_string_list(&sexp, "directories")?;
        let output = alist_string(&sexp, "output")?;
        Some(Self {
            version,
            fingerprint,
            inputs,
            directories,
            output,
        })
    }
}

fn alist_string(sexp: &Sexp, key: &str) -> Option<String> {
    alist_value(sexp, key).and_then(|value| match value {
        Sexp::String(text) => Some(text.clone()),
        _ => None,
    })
}

fn alist_string_list(sexp: &Sexp, key: &str) -> Option<Vec<String>> {
    if let Some(value) = alist_value(sexp, key) {
        return match value {
            Sexp::List(items) => string_items(items),
            _ => None,
        };
    }
    let Sexp::List(entries) = sexp else {
        return None;
    };
    entries.iter().find_map(|entry| {
        let Sexp::List(items) = entry else {
            return None;
        };
        let (head, rest) = items.split_first()?;
        if matches_symbol(head, key) {
            string_items(rest)
        } else {
            None
        }
    })
}

fn string_items(items: &[Sexp]) -> Option<Vec<String>> {
    items
        .iter()
        .map(|item| match item {
            Sexp::String(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn alist_value<'a>(sexp: &'a Sexp, key: &str) -> Option<&'a Sexp> {
    let Sexp::List(items) = sexp else {
        return None;
    };
    items.iter().find_map(|item| match item {
        Sexp::Pair(car, cdr) if matches_symbol(car, key) => Some(cdr.as_ref()),
        _ => None,
    })
}

fn matches_symbol(sexp: &Sexp, expected: &str) -> bool {
    matches!(sexp, Sexp::Symbol(symbol) if symbol == expected)
}

fn check_cache_fingerprint(root: &Path, inputs: &[String], directories: &[String]) -> String {
    format!(
        "(version: {} mode: {} inputs: ({}) directories: ({}))",
        scheme_string(CHECK_CACHE_VERSION),
        scheme_string("source-inputs"),
        inputs
            .iter()
            .map(|path| check_cache_file_fingerprint(root, path))
            .collect::<Vec<_>>()
            .join(" "),
        directories
            .iter()
            .map(|path| check_cache_file_fingerprint(root, path))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn check_cache_file_fingerprint(root: &Path, path: &str) -> String {
    let expanded = expand_project_path(root, path);
    let Ok(metadata) = fs::metadata(&expanded) else {
        return format!("({} missing)", scheme_string(path));
    };
    if metadata.is_dir() {
        let Ok(entries) = sorted_directory_entries(&expanded) else {
            return format!("({} missing)", scheme_string(path));
        };
        return format!(
            "({} directory ({}))",
            scheme_string(path),
            entries
                .iter()
                .map(|entry| scheme_string(entry))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    let Ok(hash) = fnv64_file_hash(&expanded) else {
        return format!("({} missing)", scheme_string(path));
    };
    format!("({} file {} {})", scheme_string(path), metadata.len(), hash)
}

fn sorted_directory_entries(path: &Path) -> io::Result<Vec<String>> {
    let mut entries = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}

fn fnv64_file_hash(path: &Path) -> io::Result<u64> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0_u8; 8192];
    let mut hash = FNV64_OFFSET;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hash);
        }
        for byte in &buffer[..read] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV64_PRIME);
        }
    }
}

fn expand_project_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn scheme_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    _source: &'a str,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            _source: source,
        }
    }

    fn parse(mut self) -> Result<Sexp, String> {
        let sexp = self.parse_expr()?;
        self.skip_ws();
        if self.index == self.chars.len() {
            Ok(sexp)
        } else {
            Err("trailing cache sexp input".to_string())
        }
    }

    fn parse_expr(&mut self) -> Result<Sexp, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => self.parse_list(),
            Some('"') => self.parse_string().map(Sexp::String),
            Some(_) => Ok(self.parse_atom()),
            None => Err("unexpected end of cache sexp".to_string()),
        }
    }

    fn parse_list(&mut self) -> Result<Sexp, String> {
        self.expect('(')?;
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(')') => {
                    self.index += 1;
                    return Ok(Sexp::List(items));
                }
                Some('.') if items.len() == 1 && self.dot_token_boundary() => {
                    self.index += 1;
                    let cdr = self.parse_expr()?;
                    self.skip_ws();
                    self.expect(')')?;
                    return Ok(Sexp::Pair(Box::new(items.remove(0)), Box::new(cdr)));
                }
                Some(_) => items.push(self.parse_expr()?),
                None => return Err("unterminated cache sexp list".to_string()),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut value = String::new();
        loop {
            match self.next() {
                Some('"') => return Ok(value),
                Some('\\') => match self.next() {
                    Some('n') => value.push('\n'),
                    Some('r') => value.push('\r'),
                    Some('t') => value.push('\t'),
                    Some('"') => value.push('"'),
                    Some('\\') => value.push('\\'),
                    Some(ch) => value.push(ch),
                    None => return Err("unterminated cache sexp escape".to_string()),
                },
                Some(ch) => value.push(ch),
                None => return Err("unterminated cache sexp string".to_string()),
            }
        }
    }

    fn parse_atom(&mut self) -> Sexp {
        let start = self.index;
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || ch == '(' || ch == ')' {
                break;
            }
            self.index += 1;
        }
        let token: String = self.chars[start..self.index].iter().collect();
        if let Ok(value) = token.parse::<i64>() {
            Sexp::Integer(value)
        } else {
            Sexp::Symbol(token)
        }
    }

    fn dot_token_boundary(&self) -> bool {
        self.chars
            .get(self.index + 1)
            .is_none_or(|ch| ch.is_whitespace() || *ch == ')')
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.index += 1;
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        match self.next() {
            Some(ch) if ch == expected => Ok(()),
            _ => Err(format!("expected cache sexp token `{expected}`")),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        Some(ch)
    }
}
