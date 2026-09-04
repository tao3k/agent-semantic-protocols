use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;

pub(crate) fn compile_reader_probe_artifacts() {
    println!("cargo:rerun-if-changed=reader-probe/reader_probe_fixture.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let output_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir"));
    require_success(
        run_compiler(fixture_command(&manifest_dir, &output_dir)),
        "Reader probe fixture",
    );
}

fn compiler_command(manifest_dir: &Path, output_dir: &Path) -> Command {
    let mut command = Command::new("/usr/bin/clang");
    command
        .current_dir(manifest_dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", output_dir)
        .args(["-Os", "-Wall", "-Werror"]);
    command
}

fn fixture_command(manifest_dir: &Path, output_dir: &Path) -> Command {
    let mut command = compiler_command(manifest_dir, output_dir);
    command
        .arg(manifest_dir.join("reader-probe/reader_probe_fixture.c"))
        .arg("-o")
        .arg(output_dir.join("asp-reader-probe-fixture"));
    command
}

fn run_compiler(mut command: Command) -> std::io::Result<ExitStatus> {
    command.status()
}

fn require_success(status: std::io::Result<ExitStatus>, artifact: &str) {
    let status = status.unwrap_or_else(|error| panic!("launch clang for {artifact}: {error}"));
    assert!(status.success(), "compile {artifact}");
}
