//! Local native-provider process execution for `agent-semantic-client`.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use agent_semantic_client_core::{
    ByteCount, ClientMethod, ClientReceipt, ClientRequest, ElapsedMillis, LanguageId,
    ProviderCommandReceipt, ProviderRegistrySnapshot, ResolvedProvider,
    append_syntax_query_plan_args,
};

const SEMANTIC_AGENT_PROTOCOL_BIN_ENV: &str = "SEMANTIC_AGENT_PROTOCOL_BIN";

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

type ProviderCommandOutputs = (
    ResolvedProvider,
    Vec<u8>,
    Vec<u8>,
    i32,
    Vec<ProviderCommandReceipt>,
    ElapsedMillis,
);

/// Execution backend that shells out to activated provider binaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalNativeCliBackend {
    snapshot: ProviderRegistrySnapshot,
}

impl LocalNativeCliBackend {
    pub fn new(snapshot: ProviderRegistrySnapshot) -> Self {
        Self { snapshot }
    }

    pub fn prepare(&self, request: &ClientRequest) -> Result<LocalNativeCommand, String> {
        let mut commands = self.prepare_all(request)?;
        if commands.len() == 1 {
            Ok(commands.remove(0))
        } else {
            Err("request expands to multiple provider invocations".to_string())
        }
    }

    fn prepare_all(&self, request: &ClientRequest) -> Result<Vec<LocalNativeCommand>, String> {
        let provider = self.resolve_provider(request.language_id.as_ref())?;
        Self::forwarded_arg_sets(request)
            .into_iter()
            .map(|forwarded_args| self.prepare_for_args(request, provider, forwarded_args))
            .collect()
    }

    fn prepare_for_args(
        &self,
        request: &ClientRequest,
        provider: &ResolvedProvider,
        forwarded_args: Vec<String>,
    ) -> Result<LocalNativeCommand, String> {
        let mut invocation = if let Some(runtime_command) = provider.runtime_command_prefix() {
            runtime_command
        } else if let Some(status) = provider.runtime_profile_status {
            let status = status.as_str();
            return Err(format!(
                "runtime profile for provider `{}` language `{}` is {status}; run `asp hook doctor --client codex .`",
                provider.provider_id, provider.language_id
            ));
        } else {
            provider.command_prefix()
        };
        Self::push_method(&mut invocation, &request.method)?;
        let forwarded_args = append_syntax_query_plan_args(
            &request.method,
            Some(&provider.language_id),
            forwarded_args,
        )?;
        invocation.extend(forwarded_args);
        let (program, args) = invocation
            .split_first()
            .ok_or_else(|| "provider command is empty".to_string())?;
        Ok(LocalNativeCommand {
            program: program.clone(),
            args: args.to_vec(),
            project_root: request.project_root.clone(),
            provider: provider.clone(),
        })
    }

    fn push_method(invocation: &mut Vec<String>, method: &ClientMethod) -> Result<(), String> {
        match method {
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
            | ClientMethod::CacheImport
            | ClientMethod::CacheInvalidate
            | ClientMethod::CacheFlush => {
                return Err("method is not executable by local native CLI backend".to_string());
            }
        }
        Ok(())
    }

    fn forwarded_arg_sets(request: &ClientRequest) -> Vec<Vec<String>> {
        if !Self::is_search_scope_fanout_candidate(request) {
            return vec![request.forwarded_args.clone()];
        }

        let mut scope_start = request.forwarded_args.len();
        while scope_start > 0
            && Self::is_existing_directory_arg(
                &request.project_root,
                &request.forwarded_args[scope_start - 1],
            )
        {
            scope_start -= 1;
        }
        let scopes = &request.forwarded_args[scope_start..];
        if scopes.len() <= 1 {
            return vec![request.forwarded_args.clone()];
        }

        let prefix = &request.forwarded_args[..scope_start];
        scopes
            .iter()
            .map(|scope| {
                let mut scoped = prefix.to_vec();
                scoped.push(scope.clone());
                scoped
            })
            .collect()
    }

    fn is_search_scope_fanout_candidate(request: &ClientRequest) -> bool {
        matches!(&request.method, ClientMethod::Search)
            && request
                .forwarded_args
                .first()
                .is_none_or(|subcommand| subcommand != "ingest")
    }

    fn is_existing_directory_arg(project_root: &std::path::Path, arg: &str) -> bool {
        if arg.starts_with('-') {
            return false;
        }
        let path = PathBuf::from(arg);
        let path = if path.is_absolute() {
            path
        } else {
            project_root.join(path)
        };
        path.is_dir()
    }

    pub fn execute(&self, request: &ClientRequest) -> Result<LocalNativeOutput, String> {
        let prepared_commands = self.prepare_all(request)?;
        let (provider, stdout, stderr, status_code, provider_commands, elapsed_ms) =
            Self::run_provider_commands(prepared_commands)?;
        let receipt = Self::receipt_for_run(
            request,
            &provider,
            provider_commands,
            elapsed_ms,
            stdout.len(),
            stderr.len(),
        )?;

        Ok(LocalNativeOutput {
            stdout,
            stderr,
            status_code,
            receipt,
        })
    }

    fn run_provider_commands(
        prepared_commands: Vec<LocalNativeCommand>,
    ) -> Result<ProviderCommandOutputs, String> {
        let provider = prepared_commands
            .first()
            .map(|prepared| prepared.provider.clone())
            .ok_or_else(|| "empty provider invocation set".to_string())?;
        let started_all = Instant::now();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut status_code = 0;
        let mut provider_commands = Vec::new();

        for prepared in prepared_commands {
            let started = Instant::now();
            let mut command = Command::new(&prepared.program);
            command
                .args(&prepared.args)
                .current_dir(&prepared.project_root)
                .stdin(std::process::Stdio::inherit());
            Self::set_protocol_renderer_env(&mut command);
            let output = command.output().map_err(|error| {
                format!(
                    "failed to execute provider `{}` for language `{}`: {error}",
                    prepared.provider.provider_id, prepared.provider.language_id
                )
            })?;
            let command_status = output.status.code().unwrap_or(1);
            provider_commands.push(ProviderCommandReceipt {
                language_id: prepared.provider.language_id.clone(),
                provider_id: prepared.provider.provider_id.clone(),
                argv: prepared.argv(),
                exit_code: command_status,
                stdout_bytes: ByteCount::from_len(output.stdout.len()),
                stderr_bytes: ByteCount::from_len(output.stderr.len()),
                elapsed_ms: ElapsedMillis::from_duration(started.elapsed()),
            });
            stdout.extend(output.stdout);
            stderr.extend(output.stderr);
            if command_status != 0 {
                status_code = command_status;
                break;
            }
        }

        Ok((
            provider,
            stdout,
            stderr,
            status_code,
            provider_commands,
            ElapsedMillis::from_duration(started_all.elapsed()),
        ))
    }

    fn set_protocol_renderer_env(command: &mut Command) {
        if std::env::var_os(SEMANTIC_AGENT_PROTOCOL_BIN_ENV).is_some() {
            return;
        }
        if let Ok(current_exe) = std::env::current_exe() {
            command.env(SEMANTIC_AGENT_PROTOCOL_BIN_ENV, current_exe);
        }
    }

    fn receipt_for_run(
        request: &ClientRequest,
        provider: &ResolvedProvider,
        provider_commands: Vec<ProviderCommandReceipt>,
        elapsed_ms: ElapsedMillis,
        stdout_len: usize,
        stderr_len: usize,
    ) -> Result<ClientReceipt, String> {
        let mut command_iter = provider_commands.into_iter();
        let first_command = command_iter
            .next()
            .ok_or_else(|| "empty provider command receipt set".to_string())?;
        let mut receipt = ClientReceipt::local_native(
            request.method.clone(),
            provider.provenance(),
            first_command,
        );
        receipt.provider_commands.extend(command_iter);
        receipt.provider_command_count =
            receipt.provider_commands.len().min(u32::MAX as usize) as u32;
        receipt.provider_processes_spawned = receipt.provider_command_count;
        receipt.elapsed_ms = elapsed_ms;
        receipt.stdout_bytes = ByteCount::from_len(stdout_len);
        receipt.stderr_bytes = ByteCount::from_len(stderr_len);
        Ok(receipt)
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
                .ok_or_else(|| "provider registry is empty".to_string());
        }
        Err(
            "language id is required when more than one provider is activated; use --language <id>"
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

#[allow(dead_code)]
fn prepare(_request: &ClientRequest) -> Result<LocalNativeCommand, String> {
    Err("prepare marker should not be called".to_string())
}
