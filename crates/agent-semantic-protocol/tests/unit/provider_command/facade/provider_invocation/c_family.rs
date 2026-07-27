use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn c_family_facades_route_through_ccls_asp_with_the_selected_language() {
    for language in ["c", "cpp", "objective-c"] {
        let root = temp_project_root(&format!("c-family-{language}-guide-facade"));
        let bin_dir = root.join(".bin");
        write_echo_provider(&bin_dir, "ccls-asp", "ccls");
        write_activation(
            &root,
            &[provider(
                language,
                vec![
                    bin_dir.join("ccls-asp").display().to_string(),
                    "--language".to_string(),
                    language.to_string(),
                ],
            )],
        );

        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .args([language, "guide", "."])
            .output()
            .expect("run C-family guide facade");

        assert!(
            output.status.success(),
            "language={language} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            stdout.contains(&format!("ccls args=[--language][{language}][guide]")),
            "language={language} stdout={stdout}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
