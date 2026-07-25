use super::project_shell_subject_paths;
use crate::HookRuntime;

#[test]
fn shell_subject_projection_fades_flags_and_non_file_operands() {
    let registry = HookRuntime {
        project_root: ".".to_string(),
        providers: Vec::new(),
    };
    let paths = ["-n", "-xx", "self-apply-findings.ss", "1,10p"]
        .map(str::to_string)
        .to_vec();

    assert_eq!(
        project_shell_subject_paths(&registry, &paths),
        ["self-apply-findings.ss"]
    );
}
