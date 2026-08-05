pub(crate) const RUNTIME_SERVER_MACOS_SERVICE_LABEL: &str =
    "dev.tao3k.agent-semantic-protocols.asp-runtime-server";

pub(crate) const RETIRED_MACOS_SERVICE_LABELS: &[&str] =
    &["dev.tao3k.agent-semantic-protocols.asp-resident"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeServerServiceCatalog {
    pub(crate) active_macos_label: &'static str,
    pub(crate) retired_macos_labels: &'static [&'static str],
}

pub(crate) const fn runtime_server_service_catalog() -> RuntimeServerServiceCatalog {
    RuntimeServerServiceCatalog {
        active_macos_label: RUNTIME_SERVER_MACOS_SERVICE_LABEL,
        retired_macos_labels: RETIRED_MACOS_SERVICE_LABELS,
    }
}
