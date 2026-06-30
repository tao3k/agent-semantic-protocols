#[path = "../../../src/command/gerbil_check_cache.rs"]
mod gerbil_check_cache;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CHECK_CACHE_VERSION: &str = "check-full-output-cache.v1";
const FNV64_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV64_PRIME: u64 = 1_099_511_628_211;

#[test]
fn changed_check_empty_scope_stays_in_millisecond_budget() {
    let root = temp_root("gerbil-check-changed-empty-fast-path");
    let args = vec![
        "check".to_string(),
        "changed".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ];

    let started_at = Instant::now();
    let replayed = gerbil_check_cache::try_replay_gerbil_check_cache("gerbil-scheme", &args, &root)
        .expect("changed fast path");
    let elapsed = started_at.elapsed();

    assert!(replayed);
    assert!(
        elapsed < Duration::from_millis(10),
        "Gerbil changed empty fast path exceeded 10ms: {elapsed:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn check_cache_hit_validation_stays_in_millisecond_budget() {
    let root = temp_root("gerbil-check-cache-hit-validation");
    let source_path = root.join("src/main.ss");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create src");
    fs::write(&source_path, "(def (fast-cache) 'ok)\n").expect("write source");
    let inputs = vec![source_path.display().to_string()];
    let output = "[gerbil-check] status=fail scope=full files=1 definitions=1 findings=1\n";
    write_gerbil_check_text_cache(&root, &inputs, &[], output);
    let args = vec!["check".to_string(), "--full".to_string()];

    let started_at = Instant::now();
    let replayed = gerbil_check_cache::try_replay_gerbil_check_cache("gerbil-scheme", &args, &root)
        .expect("cache replay");
    let elapsed = started_at.elapsed();

    assert!(replayed);
    assert!(
        elapsed < Duration::from_millis(50),
        "Gerbil check cache hit validation exceeded 50ms: {elapsed:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn check_cache_rejects_same_size_source_rewrite() {
    let root = temp_root("gerbil-check-cache-stale-rewrite");
    let source_path = root.join("src/main.ss");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create src");
    fs::write(&source_path, "(def same-size 'aaaa)\n").expect("write source");
    let inputs = vec![source_path.display().to_string()];
    let output = "[gerbil-check] stale output\n";
    write_gerbil_check_text_cache(&root, &inputs, &[], output);
    fs::write(&source_path, "(def same-size 'bbbb)\n").expect("rewrite source");
    let args = vec!["check".to_string(), "--full".to_string()];

    let replayed = gerbil_check_cache::try_replay_gerbil_check_cache("gerbil-scheme", &args, &root)
        .expect("cache replay");

    assert!(!replayed);
    let _ = fs::remove_dir_all(root);
}

fn write_gerbil_check_text_cache(
    root: &Path,
    inputs: &[String],
    directories: &[String],
    output: &str,
) {
    let cache_dir = root.join(".cache/agent-semantic-protocol/gerbil-scheme/check");
    fs::create_dir_all(&cache_dir).expect("create check cache dir");
    let fingerprint = gerbil_check_fingerprint(root, inputs, directories);
    let input_list = inputs
        .iter()
        .map(|path| scheme_string(path))
        .collect::<Vec<_>>()
        .join(" ");
    let directory_list = directories
        .iter()
        .map(|path| scheme_string(path))
        .collect::<Vec<_>>()
        .join(" ");
    let cache = format!(
        "((version . {version}) (fingerprint . {fingerprint}) (inputs {input_list}) (directories {directory_list}) (status . 1) (output . {output}))",
        version = scheme_string(CHECK_CACHE_VERSION),
        fingerprint = scheme_string(&fingerprint),
        output = scheme_string(output)
    );
    fs::write(cache_dir.join("text.sexp"), cache).expect("write check cache");
}

fn gerbil_check_fingerprint(root: &Path, inputs: &[String], directories: &[String]) -> String {
    format!(
        "(version: {} mode: {} inputs: ({}) directories: ({}))",
        scheme_string(CHECK_CACHE_VERSION),
        scheme_string("source-inputs"),
        inputs
            .iter()
            .map(|path| gerbil_file_fingerprint(root, path))
            .collect::<Vec<_>>()
            .join(" "),
        directories
            .iter()
            .map(|path| gerbil_file_fingerprint(root, path))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn gerbil_file_fingerprint(root: &Path, path: &str) -> String {
    let path_buf = PathBuf::from(path);
    let expanded = if path_buf.is_absolute() {
        path_buf
    } else {
        root.join(path_buf)
    };
    let metadata = fs::metadata(&expanded).expect("fingerprint metadata");
    if metadata.is_dir() {
        let entries = sorted_directory_entries(&expanded);
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
    format!(
        "({} file {} {})",
        scheme_string(path),
        metadata.len(),
        fnv64_file_hash(&expanded)
    )
}

fn sorted_directory_entries(path: &Path) -> Vec<String> {
    let mut entries = fs::read_dir(path)
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn fnv64_file_hash(path: &Path) -> u64 {
    fs::read(path)
        .expect("read fingerprint file")
        .into_iter()
        .fold(FNV64_OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV64_PRIME)
        })
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

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-protocol-{name}-{unique}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}
