use std::path::PathBuf;

use agent_semantic_schema_manager::SchemaManager;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let operation = arguments.next().unwrap_or_else(|| "help".to_owned());
    if matches!(operation.as_str(), "help" | "--help" | "-h") {
        println!(
            "usage: asp-schema-manager <materialize|verify> --workspace <ROOT> [--language <ID>]..."
        );
        return Ok(());
    }
    let mut workspace = PathBuf::from(".");
    let mut languages = Vec::new();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--workspace" => {
                workspace = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--workspace requires a path".to_owned())?,
                );
            }
            "--language" => languages.push(
                arguments
                    .next()
                    .ok_or_else(|| "--language requires an identity".to_owned())?,
            ),
            other => return Err(format!("unknown asp-schema-manager argument: {other}")),
        }
    }
    let manager = SchemaManager::new(workspace);
    let reports = match operation.as_str() {
        "materialize" => manager.materialize(&languages).await?,
        "verify" => manager.verify(&languages).await?,
        other => return Err(format!("unknown asp-schema-manager operation: {other}")),
    };
    for report in reports {
        println!(
            "[schema-bundle] language={} schemas={} changed={} removed={} digest={} receipt={}",
            report.language_id,
            report.schema_count,
            report.changed_count,
            report.removed_count,
            report.bundle_digest,
            report.receipt_path.display()
        );
    }
    Ok(())
}
