use std::future::Future;
use std::sync::OnceLock;

fn runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_name("asp-workspace-resident")
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create workspace DB runtime: {error}"))?;
    let _ = RUNTIME.set(runtime);
    RUNTIME
        .get()
        .ok_or_else(|| "workspace DB runtime initialization did not publish".to_owned())
}

pub(super) fn block_on<F: Future>(future: F) -> Result<F::Output, String> {
    Ok(runtime()?.block_on(future))
}

pub(super) fn handle() -> Result<&'static tokio::runtime::Runtime, String> {
    runtime()
}
