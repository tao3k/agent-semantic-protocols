use super::{
    ASP_QUERY_WRAPPER_WALL_SANITY_GATE, asp_command, assert_trace_elapsed_under_gate,
    temp_project_root, write_regular_search_fixtures,
};
use std::time::Instant;

#[test]
fn query_wrapper_commands_finish_inside_search_phase_gate() {
    let root = temp_project_root("query-wrapper-performance-gate");
    write_regular_search_fixtures(&root);

    let command_suite = [
        [
            "fd",
            "-query",
            "RustGate|typescriptGate|python_gate|julia_gate|gerbil-gate",
            ".",
        ],
        [
            "rg",
            "-query",
            "RustGate typescriptGate python_gate julia_gate gerbil-gate",
            ".",
        ],
    ];
    for args in command_suite {
        let started_at = Instant::now();
        let output = asp_command(&root)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run asp {args:?}: {error}"));
        let elapsed = started_at.elapsed();
        assert!(
            output.status.success(),
            "args={args:?} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            elapsed < ASP_QUERY_WRAPPER_WALL_SANITY_GATE,
            "asp {args:?} exceeded wrapper wall sanity gate {ASP_QUERY_WRAPPER_WALL_SANITY_GATE:?}; elapsed={elapsed:?}; stdout={stdout}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("sourceTrace=") && stdout.contains("elapsedMs="),
            "args={args:?} stdout={stdout}"
        );
        assert_trace_elapsed_under_gate(&args, &stdout);
    }
    let _ = std::fs::remove_dir_all(root);
}
