#![allow(dead_code)]

#[path = "../../src/command/hook.rs"]
mod hook;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn install_delegates_to_hook_install() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&["install", "--client", "codex", "."])).unwrap(),
        args(&["install", "--client", "codex", "."])
    );
}

#[test]
fn event_alias_delegates_to_hook_runtime() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&["pre-tool", "--client", "codex"])).unwrap(),
        args(&["hook", "--event", "pre-tool", "--client", "codex"])
    );
}

#[test]
fn raw_hook_flags_stay_supported() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&["--client", "codex", "--event", "stop"])).unwrap(),
        args(&["hook", "--client", "codex", "--event", "stop"])
    );
}
