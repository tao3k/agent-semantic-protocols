use std::path::Path;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeServerStartupReceipt<'a> {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub owner_epoch: u64,
    pub stage: &'a str,
    pub state: &'a str,
    pub elapsed_micros: u128,
    pub error: Option<&'a str>,
}

pub(crate) async fn publish(
    state_home: &Path,
    owner_epoch: u64,
    stage: &'static str,
    state: &'static str,
    started: tokio::time::Instant,
    error: Option<&str>,
) -> Result<(), String> {
    let target = state_home.join("runtime/server/daemon-startup.v1.json");
    let parent = target
        .parent()
        .ok_or_else(|| "startup receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| format!("create startup receipt parent: {e}"))?;
    let staged = target.with_extension(format!("json.stage-{}", std::process::id()));
    let bytes = serde_json::to_vec(&RuntimeServerStartupReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-startup-receipt.v1",
        schema_version: "1",
        owner_epoch,
        stage,
        state,
        elapsed_micros: started.elapsed().as_micros(),
        error,
    })
    .map_err(|e| format!("encode startup receipt: {e}"))?;
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(&staged)
        .await
        .map_err(|e| format!("create startup receipt: {e}"))?;
    file.write_all(&bytes)
        .await
        .map_err(|e| format!("write startup receipt: {e}"))?;
    file.sync_all()
        .await
        .map_err(|e| format!("sync startup receipt: {e}"))?;
    drop(file);
    tokio::fs::rename(&staged, &target)
        .await
        .map_err(|e| format!("publish startup receipt: {e}"))
}
