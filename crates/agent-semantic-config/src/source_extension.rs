//! Provider-owned source-extension matching.

use std::path::Path;

/// Return whether provider-schema source extensions admit the file at `path`.
#[must_use]
pub fn source_extensions_support_file(source_extensions: &[String], path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    source_extensions.iter().any(|candidate| {
        candidate
            .trim_start_matches('.')
            .eq_ignore_ascii_case(extension)
    })
}
