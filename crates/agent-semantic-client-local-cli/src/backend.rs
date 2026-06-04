//! Local native-provider process execution for `agent-semantic-client`.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use agent_semantic_client_core::{
    ByteCount, ClientMethod, ClientReceipt, ClientRequest, ElapsedMillis, LanguageId,
    ProviderCommandReceipt, ProviderRegistrySnapshot, ResolvedProvider,
};

/// Prepared native provider command built from activation data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalNativeCommand {
    pub program: String,
    pub args: Vec<String>,
    pub project_root: PathBuf,
    pub provider: ResolvedProvider,
}

/// Captured output and receipt metadata for a provider command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalNativeOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status_code: i32,
    pub receipt: ClientReceipt,
}

/// Execution backend that shells out to activated provider binaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalNativeCliBackend {
    snapshot: ProviderRegistrySnapshot,
}

impl LocalNativeCliBackend {
    /// Create a backend over one provider registry snapshot.
    #[must_use]
    pub fn new(snapshot: ProviderRegistrySnapshot) -> Self {
        Self { snapshot }
    }

    /// Build the provider command without running it.
    pub fn prepare(&self, request: &ClientRequest) -> Result<LocalNativeCommand, String> {
        let provider = self.resolve_provider(request.language_id.as_ref())?;
        let mut invocation = provider.command_prefix();

        match request.method {
            ClientMethod::Search => invocation.push("search".to_string()),
            ClientMethod::Query => invocation.push("query".to_string()),
            ClientMethod::Check => invocation.push("check".to_string()),
            ClientMethod::Guide => {
                invocation.push("agent".to_string());
                invocation.push("guide".to_string());
            }
            ClientMethod::Providers
            | ClientMethod::Doctor
            | ClientMethod::CacheStatus
            | ClientMethod::CacheImport => {
                return Err(format!(
                    "`{:?}` is handled by agent-semantic-client, not LocalNativeCliBackend",
                    request.method
                ));
            }
        }

        invocation.extend(request.forwarded_args.clone());
        let (program, args) = invocation
            .split_first()
            .ok_or_else(|| "provider command prefix is empty".to_string())?;

        Ok(LocalNativeCommand {
            program: program.clone(),
            args: args.to_vec(),
            project_root: request.project_root.clone(),
            provider: provider.clone(),
        })
    }

    /// Run the provider command and capture stdout, stderr, status, and receipt data.
    pub fn execute(&self, request: &ClientRequest) -> Result<LocalNativeOutput, String> {
        let prepared = self.prepare(request)?;
        let started = Instant::now();
        let output = Command::new(&prepared.program)
            .args(&prepared.args)
            .current_dir(&prepared.project_root)
            .output()
            .map_err(|error| {
                format!(
                    "failed to spawn provider `{}` for language `{}`: {error}",
                    prepared.provider.provider_id, prepared.provider.language_id
                )
            })?;
        let status_code = output.status.code().unwrap_or(1);
        let provider_command = ProviderCommandReceipt {
            language_id: prepared.provider.language_id.clone(),
            provider_id: prepared.provider.provider_id.clone(),
            argv: prepared.argv(),
            exit_code: status_code,
            stdout_bytes: ByteCount::from_len(output.stdout.len()),
            stderr_bytes: ByteCount::from_len(output.stderr.len()),
            elapsed_ms: ElapsedMillis::from_duration(started.elapsed()),
        };
        Ok(LocalNativeOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            status_code,
            receipt: ClientReceipt::local_native(
                request.method.clone(),
                prepared.provider.provenance(),
                provider_command,
            ),
        })
    }

    fn resolve_provider(
        &self,
        language_id: Option<&LanguageId>,
    ) -> Result<&ResolvedProvider, String> {
        if let Some(language_id) = language_id {
            return self
                .snapshot
                .provider_for_language(language_id)
                .ok_or_else(|| format!("no activated provider for language `{language_id}`"));
        }

        if self.snapshot.providers.len() == 1 {
            return self
                .snapshot
                .providers
                .first()
                .ok_or_else(|| "activation has no providers".to_string());
        }

        Err(
            "language is required when multiple providers are activated; use --language <id>"
                .to_string(),
        )
    }
}

impl LocalNativeCommand {
    /// Return full argv including program and arguments.
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        let mut argv = Vec::with_capacity(self.args.len() + 1);
        argv.push(self.program.clone());
        argv.extend(self.args.clone());
        argv
    }
}
