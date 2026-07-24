use crate::provider_command::support;

pub(super) fn refresh_source_index(root: &std::path::Path) {
    let output = support::asp_command(root)
        .args(["cache", "source-index", "rebuild"])
        .output()
        .expect("run asp cache source-index rebuild");
    assert!(
        output.status.success(),
        "source-index rebuild failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
