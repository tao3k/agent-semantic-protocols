#[cfg(test)]
mod cli_help_tests {
    use std::process::Command;

    fn assert_standard_help(args: &[&str]) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run asp {}: {error}", args.join(" ")));
        assert!(
            output.status.success(),
            "args={args:?} stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(stdout.contains("Usage:"), "args={args:?} stdout={stdout}");
        assert!(stdout.contains("Options:"), "args={args:?} stdout={stdout}");
        assert!(
            !stdout.starts_with("usage:"),
            "help must come from the standard clap renderer: args={args:?} stdout={stdout}"
        );
        stdout
    }

    #[test]
    fn public_first_level_help_uses_standard_renderer() {
        for args in [
            &["--help"][..],
            &["guide", "--help"],
            &["providers", "--help"],
            &["tools", "--help"],
            &["wrap", "--help"],
            &["cache", "--help"],
            &["cloud", "--help"],
            &["hook", "--help"],
            &["agent", "--help"],
            &["install", "--help"],
            &["sync", "--help"],
            &["paths", "--help"],
            &["healthcheck", "--help"],
            &["source-access", "--help"],
            &["ast-patch", "--help"],
            &["graph", "--help"],
            &["fd", "--help"],
            &["rg", "--help"],
            &["search", "--help"],
            &["query", "--help"],
            &["gerbil-scheme", "--help"],
            &["julia", "--help"],
            &["md", "--help"],
            &["org", "--help"],
            &["python", "--help"],
            &["rust", "--help"],
            &["typescript", "--help"],
        ] {
            assert_standard_help(args);
        }
    }

    #[test]
    fn representative_nested_help_uses_standard_renderer() {
        for args in [
            &["install", "plugin", "--codex", "--help"][..],
            &["install", "language", "--help"],
            &["hook", "doctor", "--help"],
            &["agent", "session", "--help"],
            &["graph", "render", "--help"],
            &["rust", "search", "--help"],
        ] {
            assert_standard_help(args);
        }
    }
}
