pub(crate) fn run_runtime_server_command(args: &[String]) -> Result<(), String> {
    crate::server::runtime_server::run_runtime_server_command(args)
}
