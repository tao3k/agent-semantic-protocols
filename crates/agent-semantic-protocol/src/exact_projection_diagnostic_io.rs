use crate::exact_projection_diagnostic::{
    ProviderNativeExactResolution, render_provider_exact_resolution, validate_resolution,
};

pub(crate) async fn emit_resolution(
    resolution: &ProviderNativeExactResolution,
    language_id: &str,
    provider_id: &str,
    owner_path: &str,
    structural_selector: &str,
) -> Result<(), String> {
    validate_resolution(
        resolution,
        language_id,
        provider_id,
        owner_path,
        structural_selector,
    )?;
    Err(render_provider_exact_resolution(resolution))
}

pub(crate) async fn write_stdout(bytes: &[u8], subject: &str) -> Result<(), String> {
    use tokio::io::AsyncWriteExt as _;
    let mut stdout = tokio::io::stdout();
    stdout
        .write_all(bytes)
        .await
        .map_err(|error| format!("failed to write {subject}: {error}"))?;
    stdout
        .flush()
        .await
        .map_err(|error| format!("failed to flush {subject}: {error}"))
}
