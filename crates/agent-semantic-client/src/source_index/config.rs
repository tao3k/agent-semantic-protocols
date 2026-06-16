//! Static limits and filename catalogs for Rust SQL source indexing.

pub(super) const SOURCE_INDEX_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-source-index";
pub(super) const SOURCE_INDEX_SCHEMA_VERSION: &str = "1";
pub(super) const SOURCE_INDEX_PROVIDER_ID: &str = "rust-sql-source-index";
pub(super) const SOURCE_INDEX_QUERY_KEY_LIMIT: usize = 128;
pub(super) const SOURCE_INDEX_FILE_LIMIT: usize = 4096;
pub(super) const SOURCE_INDEX_FILE_BYTES_LIMIT: u64 = 1_048_576;
pub(super) const SOURCE_INDEX_PROJECT_ANCHOR_FILENAMES: &[&str] = &[
    "Cargo.toml",
    "pyproject.toml",
    "package.json",
    "Project.toml",
    "gerbil.pkg",
];
pub(super) const SOURCE_INDEX_SKIP_DIRS: &[&str] = &[
    ".codex",
    ".data",
    ".devenv",
    ".direnv",
    ".git",
    ".cache",
    ".gerbil",
    ".jj",
    ".venv",
    "node_modules",
    "target",
    "dist",
    "build",
];
pub(super) const SOURCE_INDEX_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "jl", "ss", "ssi", "scm", "sld", "org", "md",
];
pub(super) const SOURCE_INDEX_CONFIG_FILENAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "pnpm-workspace.yaml",
    "pyproject.toml",
    "Project.toml",
    "gerbil.pkg",
    "build.ss",
];
