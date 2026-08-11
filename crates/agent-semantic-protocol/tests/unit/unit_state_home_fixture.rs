use std::path::{Path, PathBuf};

pub(crate) fn default_state_home(root: &Path) -> PathBuf {
    root.join(".agent-semantic-protocols")
}

pub(crate) fn materialize_org_state_checkout(state_home: &Path) {
    let org_state = state_home.join("org");
    let output = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .arg(&org_state)
        .output()
        .expect("initialize fixture Org state checkout");
    assert!(
        output.status.success(),
        "initialize fixture Org state checkout: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let skills_dir = org_state.join("skills");
    std::fs::create_dir_all(&skills_dir).expect("create fixture Org skills directory");
    std::fs::write(skills_dir.join("ASP_ORG.org"), "* ASP Org fixture\n")
        .expect("write fixture Org skill");
}
