use agent_semantic_schema_manager::run_cli;

#[tokio::main]
async fn main() {
    if let Err(error) = run_cli(std::env::args().skip(1)).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
