use agent_semantic_hook::{
    build_default_activation, default_activation_path, load_or_sync_activation, write_activation,
};
use std::fs;

use super::{make_executable, temp_root};

#[test]
fn generated_activation_sync_refreshes_newly_available_parent_workspace_provider() {
    let root = temp_root("nested-gerbil-refresh-parent-bin-provider");
    let child = root
        .join("languages")
        .join("gerbil-scheme-language-project-harness");
    fs::create_dir_all(root.join(".bin")).expect("create workspace bin");
    fs::create_dir_all(child.join("src")).expect("create child src");
    fs::write(child.join("gerbil.pkg"), "(package: sample/gerbil)\n").expect("write gerbil.pkg");
    fs::write(root.join("asp.toml"), "[providers]\n").expect("write workspace asp.toml");
    fs::write(
        child.join("asp.toml"),
        "[providers.gerbil-scheme]\nenabled = false\n",
    )
    .expect("write initial child asp.toml");
    let asp_bin = root.join(".bin/asp");
    fs::write(&asp_bin, "#!/bin/sh\nexit 0\n").expect("write asp bin");
    make_executable(&asp_bin);
    let activation_path = default_activation_path(&child);
    let initial_activation = build_default_activation(&child).expect("build initial activation");
    assert!(
        !initial_activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "gerbil-scheme")
    );
    write_activation(&activation_path, &initial_activation).expect("write old activation");

    fs::remove_file(child.join("asp.toml")).expect("enable default child providers");
    let gerbil_bin = root.join(".bin/gerbil-scheme-harness");
    fs::write(&gerbil_bin, "#!/bin/sh\nexit 0\n").expect("write gerbil provider bin");
    make_executable(&gerbil_bin);

    let runtime = load_or_sync_activation(&activation_path, &child).expect("sync activation");
    assert!(
        runtime
            .providers
            .iter()
            .any(|provider| provider.language_id == "gerbil-scheme"),
        "generated activation should refresh when a parent workspace Gerbil provider becomes available"
    );
    let refreshed_activation = fs::read_to_string(&activation_path).expect("read refreshed");
    assert!(refreshed_activation.contains("\"languageId\": \"gerbil-scheme\""));

    fs::remove_dir_all(root).expect("remove temp root");
}
