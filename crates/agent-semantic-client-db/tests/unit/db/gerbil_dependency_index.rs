use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::{
    DEFAULT_GERBIL_DEPS_SEARCH_LIMIT, GerbilDepsQueryRequest, GerbilDepsSearchRequest,
    gerbil_deps_query_export, gerbil_deps_query_terms, gerbil_deps_search_exports,
};

use crate::env::ENV_LOCK;

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: OsString) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match self.previous.as_ref() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[test]
fn gerbil_deps_core_search_is_millisecond_scale() {
    let fixture = GerbilFixture::write("gerbil-deps-core-search");
    let _env_lock = ENV_LOCK.lock().expect("lock env");
    let _path_guard = EnvVarGuard::set("PATH", prepend_path(&fixture.bin_dir));
    let request = GerbilDepsSearchRequest {
        module_id: ":std/srfi/13".to_string(),
        query: "string-prefix".to_string(),
        terms: gerbil_deps_query_terms("string-prefix"),
        limit: DEFAULT_GERBIL_DEPS_SEARCH_LIMIT,
    };

    let start = Instant::now();
    let result = gerbil_deps_search_exports(&request).expect("search exports");
    let elapsed = start.elapsed();

    assert_eq!(
        result.exports,
        vec![
            "string-prefix?".to_string(),
            "string-prefix-ci?".to_string()
        ]
    );
    assert!(
        elapsed < Duration::from_millis(20),
        "Gerbil deps core search should stay millisecond-scale, elapsed={elapsed:?}"
    );

    let _ = std::fs::remove_dir_all(fixture.root);
}

#[test]
fn gerbil_deps_core_query_follows_local_include() {
    let fixture = GerbilFixture::write("gerbil-deps-core-query");
    let _env_lock = ENV_LOCK.lock().expect("lock env");
    let _path_guard = EnvVarGuard::set("PATH", prepend_path(&fixture.bin_dir));
    let request = GerbilDepsQueryRequest {
        selector: "gerbil:/std/srfi/13#export/string-prefix?".to_string(),
        module_id: ":std/srfi/13".to_string(),
        export_name: "string-prefix?".to_string(),
    };

    let result = gerbil_deps_query_export(&request).expect("query export");

    assert_eq!(result.source_line, Some(1));
    assert!(result.source_text.contains("(def (string-prefix? s1 s2"));
    assert!(result.source_path.ends_with("srfi-13.scm"));

    let _ = std::fs::remove_dir_all(fixture.root);
}

struct GerbilFixture {
    root: PathBuf,
    bin_dir: PathBuf,
}

impl GerbilFixture {
    fn write(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "agent-semantic-client-db-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let prefix = root.join("gerbil-prefix");
        let bin_dir = prefix.join("bin");
        let source_dir = prefix.join("v0.18.2/src/std/srfi");
        std::fs::create_dir_all(&bin_dir).expect("create fixture bin dir");
        std::fs::create_dir_all(&source_dir).expect("create fixture source dir");
        let gxi = bin_dir.join("gxi");
        std::fs::write(&gxi, "#!/bin/sh\nexit 0\n").expect("write fake gxi");
        make_executable(&gxi);
        std::fs::write(
            source_dir.join("13.ss"),
            r#"(export
  string-prefix-length string-prefix-length-ci
  string-prefix? string-prefix-ci?
  string-suffix? string-suffix-ci?)
(include "srfi-13.scm")
"#,
        )
        .expect("write module");
        std::fs::write(
            source_dir.join("srfi-13.scm"),
            r#"(def (string-prefix? s1 s2
                     (start1 0) (end1 (string-length s1))
                     (start2 0) (end2 (string-length s2)))
  (%string-prefix? s1 start1 end1 s2 start2 end2))
"#,
        )
        .expect("write include");
        Self { root, bin_dir }
    }
}

fn prepend_path(path_prefix: &Path) -> OsString {
    let mut paths = vec![path_prefix.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths).expect("join PATH")
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("fixture metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod fixture");
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
