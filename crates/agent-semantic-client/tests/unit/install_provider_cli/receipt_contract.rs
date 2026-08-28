#[test]
fn root_justfile_develop_install_is_only_a_registry_driven_adapter() {
    let justfile = include_str!("../../../../../Justfile");

    for required in [
        r#"protocol_bin="${state_home}/runtime/bin/asp""#,
        r#"install language "{{ language }}""#,
        "installMode=provider-workspace-install source=provider-registry",
    ] {
        assert!(
            justfile.contains(required),
            "developer provider adapter lost its registry contract: {required}"
        );
    }
    for forbidden in [
        "--record-installed-receipt",
        "provider_source=",
        r#"case "{{ language }}" in"#,
        "languages/rust-lang-project-harness/target/release/asp-rust",
        "runtime/provider-artifacts/asp-gerbil-scheme/develop",
    ] {
        assert!(
            !justfile.contains(forbidden),
            "root Justfile duplicated provider-owned install logic: {forbidden}"
        );
    }
}
