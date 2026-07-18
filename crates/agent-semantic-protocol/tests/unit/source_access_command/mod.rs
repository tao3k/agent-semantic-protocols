#[path = "../../../src/command/source_access.rs"]
pub(super) mod source_access;

const _: fn(&[String]) -> Result<(), String> = source_access::run_source_access_command;

mod errors;
mod happy_path;
mod support;
