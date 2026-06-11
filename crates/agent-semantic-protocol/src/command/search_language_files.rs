//! Language-owned source and config file boundaries for ASP search surfaces.

use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LanguageFileSpec {
    extensions: &'static [&'static str],
    config_filenames: &'static [&'static str],
}

impl LanguageFileSpec {
    pub(super) fn extensions(self) -> &'static [&'static str] {
        self.extensions
    }

    pub(super) fn config_filenames(self) -> &'static [&'static str] {
        self.config_filenames
    }

    pub(super) fn matches(self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| self.extensions.contains(&extension))
            || self.is_config_path(path)
    }

    pub(super) fn is_config_path(self, path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| self.config_filenames.contains(&name))
    }
}

pub(super) fn language_file_spec(language_id: &str) -> LanguageFileSpec {
    match language_id {
        "rust" => LanguageFileSpec {
            extensions: &["rs"],
            config_filenames: &["Cargo.toml"],
        },
        "typescript" => LanguageFileSpec {
            extensions: &["ts", "tsx", "js", "jsx"],
            config_filenames: &["package.json", "tsconfig.json", "pnpm-workspace.yaml"],
        },
        "python" => LanguageFileSpec {
            extensions: &["py"],
            config_filenames: &["pyproject.toml"],
        },
        "julia" => LanguageFileSpec {
            extensions: &["jl"],
            config_filenames: &["Project.toml"],
        },
        "gerbil-scheme" => LanguageFileSpec {
            extensions: &["ss", "ssi", "scm", "sld"],
            config_filenames: &["gerbil.pkg", "build.ss"],
        },
        _ => LanguageFileSpec {
            extensions: &[],
            config_filenames: &[],
        },
    }
}
