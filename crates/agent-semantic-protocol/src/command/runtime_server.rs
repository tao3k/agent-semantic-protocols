pub(crate) fn run_runtime_server_command(args: &[String]) -> Result<(), String> {
    crate::server::runtime_server::run_runtime_server_command(args)
}

pub(crate) fn runtime_server_hook_evaluation_client(
    project_root: &std::path::Path,
    arguments: Vec<String>,
    input: String,
) -> Result<String, String> {
    crate::server::runtime_server::runtime_server_hook_evaluation_client(
        project_root,
        arguments,
        input,
    )
}
