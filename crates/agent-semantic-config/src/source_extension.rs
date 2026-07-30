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

#[cfg(test)]
mod tests {
    use super::source_extensions_support_file;
    use std::path::Path;

    #[test]
    fn provider_schema_extensions_accept_dotless_and_dotted_names() {
        let extensions = vec!["rs".to_string(), ".py".to_string()];

        assert!(source_extensions_support_file(
            &extensions,
            Path::new("src/LIB.RS")
        ));
        assert!(source_extensions_support_file(
            &extensions,
            Path::new("src/main.py")
        ));
        assert!(!source_extensions_support_file(
            &extensions,
            Path::new("src/main.ts")
        ));
        assert!(!source_extensions_support_file(
            &extensions,
            Path::new("Makefile")
        ));
    }
}
