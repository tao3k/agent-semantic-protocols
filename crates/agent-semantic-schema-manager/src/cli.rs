//! Thin command adapter for Schema Manager materialization and publication.

use std::path::PathBuf;

use crate::SchemaManager;

pub async fn run_cli(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let operation = arguments.next().unwrap_or_else(|| "help".to_owned());
    if matches!(operation.as_str(), "help" | "--help" | "-h") {
        println!(
            "usage: asp-schema-manager <materialize|verify|responsibilities|publish-client> --workspace <ROOT> [--language <ID>]... [--output <DIR>]"
        );
        return Ok(());
    }
    let mut workspace = PathBuf::from(".");
    let mut languages = Vec::new();
    let mut output = None;
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
            "--output" => {
                output = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--output requires a path".to_owned())?,
                ));
            }
            other => return Err(format!("unknown asp-schema-manager argument: {other}")),
        }
    }
    let manager = SchemaManager::new(workspace);
    if operation == "responsibilities" {
        if !languages.is_empty() || output.is_some() {
            return Err("responsibilities does not accept --language or --output".to_owned());
        }
        for responsibility in manager.responsibilities().await? {
            println!(
                "[schema-responsibility] schema={} schemaId={} family={} owner={} purpose={}",
                responsibility.name,
                responsibility.schema_id,
                responsibility.family_id,
                responsibility.owner,
                responsibility.purpose
            );
        }
        return Ok(());
    }
    let reports = match operation.as_str() {
        "materialize" => manager.materialize(&languages).await?,
        "verify" => manager.verify(&languages).await?,
        "publish-client" => {
            if languages.len() != 1 {
                return Err("publish-client requires exactly one --language".to_owned());
            }
            let output = output.ok_or_else(|| "publish-client requires --output".to_owned())?;
            vec![
                manager
                    .publish_client_bundle(languages.remove(0), output)
                    .await?,
            ]
        }
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
