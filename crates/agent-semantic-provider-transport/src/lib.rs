#![deny(dead_code)]

//! Process transport for external ASP language providers.

pub mod byte_text;
mod capture;
mod process_contract;
mod transport;

pub use process_contract::{
    OutputFraming, OutputMode, ProviderProcessError, ProviderProcessFraming, ProviderProcessLimits,
    ProviderProcessReceipt, ProviderProcessSpec, StdinMode,
};
pub use transport::{
    ProviderProcessOutput, provider_process_limits_from_environment, run_provider_process,
    run_provider_process_async, run_provider_process_async_with_framing,
    run_provider_process_with_framing,
};
