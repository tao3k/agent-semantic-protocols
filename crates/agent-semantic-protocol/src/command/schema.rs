use std::path::PathBuf;

use agent_semantic_schema_manager::SchemaManager;

pub(super) async fn run_schema_command(args: &[String]) -> Result<(), String> {
    let Some(operation) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    if matches!(operation, "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    let mut workspace = PathBuf::from(".");
    let mut languages = Vec::new();
    let mut output: Option<PathBuf> = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--workspace" => {
                index += 1;
                workspace = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "asp schema --workspace requires a path".to_owned())?,
                );
            }
            "--language" => {
                index += 1;
                languages.push(
                    args.get(index)
                        .ok_or_else(|| "asp schema --language requires an identity".to_owned())?
                        .clone(),
                );
            }
            "--output" => {
                index += 1;
                output =
                    Some(PathBuf::from(args.get(index).ok_or_else(|| {
                        "asp schema --output requires a path".to_owned()
                    })?));
            }
            argument => return Err(format!("unknown asp schema argument: {argument}")),
        }
        index += 1;
    }
    let manager = SchemaManager::new(workspace);
    if operation == "responsibilities" {
        if !languages.is_empty() {
            return Err("asp schema responsibilities does not accept --language".to_owned());
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
    let reports = match operation {
        "publish-client" => {
            if languages.len() != 1 {
                return Err("asp schema publish-client requires exactly one --language".to_owned());
            }
            let output =
                output.ok_or_else(|| "asp schema publish-client requires --output".to_owned())?;
            vec![
                manager
                    .publish_client_bundle(languages[0].clone(), output)
                    .await?,
            ]
        }
        "materialize" => manager.materialize(&languages).await?,
        "verify" => manager.verify(&languages).await?,
        other => {
            return Err(format!(
                "unknown asp schema operation: {other}\n{}",
                usage()
            ));
        }
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

fn usage() -> String {
    "usage: asp schema <materialize|verify|publish-client|responsibilities> [--workspace ROOT] [--language ID] [--output DIR]"
        .to_owned()
}
