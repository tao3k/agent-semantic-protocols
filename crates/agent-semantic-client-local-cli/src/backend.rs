//! Local native-provider process execution for `agent-semantic-client`.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;

use agent_semantic_client_core::{
    ByteCount, ClientMethod, ClientReceipt, ClientRequest, ElapsedMillis, LanguageId,
    ProviderCommandReceipt, ProviderRegistrySnapshot, ResolvedProvider,
    append_syntax_query_plan_args,
};
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode,
    provider_process_limits_from_environment, run_provider_process as run_transport_process,
};
use bytes::{Bytes, BytesMut};

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
    pub stdout: Bytes,
    pub stderr: Bytes,
    pub status_code: i32,
    pub receipt: ClientReceipt,
}

type ProviderCommandOutputs = (
    ResolvedProvider,
    Bytes,
    Bytes,
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
        let project_root = provider_process_cwd(&request.project_root)?;
        let mut invocation = Self::provider_command_prefix(provider)?;
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
            project_root,
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

    fn home_local_provider_binary(provider: &ResolvedProvider) -> Result<String, String> {
        let home = env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "provider binary `{}` for language `{}` must be installed at $HOME/.local/bin/{}; HOME is not set",
                    provider.binary, provider.language_id, provider.binary
                )
            })?;
        let path = Path::new(&home).join(".local/bin").join(&provider.binary);
        if !path.is_file() {
            return Err(format!(
                "provider binary `{}` for language `{}` must be installed at {}; run `asp install language {}`",
                provider.binary,
                provider.language_id,
                path.display(),
                provider.language_id
            ));
        }
        Ok(path.to_string_lossy().to_string())
    }

    fn provider_command_prefix(provider: &ResolvedProvider) -> Result<Vec<String>, String> {
        let prefix = vec![Self::home_local_provider_binary(provider)?];
        Ok(Self::provider_command_prefix_with_facade_language(
            provider, prefix,
        ))
    }

    fn provider_command_prefix_with_facade_language(
        provider: &ResolvedProvider,
        mut prefix: Vec<String>,
    ) -> Vec<String> {
        if Self::provider_command_prefix_needs_facade_language(provider, &prefix) {
            prefix.insert(1, provider.language_id.to_string());
        }
        prefix
    }

    fn same_binary_path(left: &str, right: &Path) -> bool {
        let left = Path::new(left);
        let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
        let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
        left == right
    }

    fn provider_command_prefix_needs_facade_language(
        provider: &ResolvedProvider,
        prefix: &[String],
    ) -> bool {
        let Some(program) = prefix.first() else {
            return false;
        };
        let language_id = provider.language_id.to_string();
        if prefix.get(1).is_some_and(|arg| arg == &language_id) {
            return false;
        }
        if provider.binary == "asp" && Self::program_name_is_asp(program) {
            return true;
        }
        env::var_os(SEMANTIC_AGENT_PROTOCOL_BIN_ENV)
            .is_some_and(|protocol_bin| Self::same_binary_path(program, Path::new(&protocol_bin)))
    }

    fn program_name_is_asp(program: &str) -> bool {
        Path::new(program)
            .file_name()
            .is_some_and(|name| name.to_string_lossy() == "asp")
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
        self.execute_with_limits(request, provider_process_limits_from_environment()?)
    }

    pub fn execute_with_limits(
        &self,
        request: &ClientRequest,
        limits: ProviderProcessLimits,
    ) -> Result<LocalNativeOutput, String> {
        let prepared_commands = self.prepare_all(request)?;
        let (provider, stdout, stderr, status_code, provider_commands, elapsed_ms) =
            Self::run_provider_commands(prepared_commands, request.stdin.clone(), limits)?;
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
        stdin: Option<Bytes>,
        limits: ProviderProcessLimits,
    ) -> Result<ProviderCommandOutputs, String> {
        let provider = prepared_commands
            .first()
            .map(|prepared| prepared.provider.clone())
            .ok_or_else(|| "empty provider invocation set".to_string())?;
        let started_all = Instant::now();
        let mut stdout = BytesMut::new();
        let mut stderr = BytesMut::new();
        let mut status_code = 0;
        let mut provider_commands = Vec::new();

        for prepared in prepared_commands {
            let provider_argv = prepared.argv();
            let provider_cwd = prepared.project_root.clone();
            let output = run_transport_process(ProviderProcessSpec {
                program: prepared.program.clone(),
                args: prepared.args.clone(),
                cwd: prepared.project_root.clone(),
                env: Self::protocol_renderer_env(),
                stdin: stdin
                    .clone()
                    .map(StdinMode::bytes)
                    .unwrap_or(StdinMode::Inherit),
                stdout: OutputMode::Capture,
                stderr: OutputMode::Capture,
                limits,
            })
            .map_err(|error| {
                format!(
                    "failed to execute provider `{}` for language `{}` with cwd `{}` argv `{}`: {error}",
                    prepared.provider.provider_id,
                    prepared.provider.language_id,
                    provider_cwd.display(),
                    provider_argv.join(" ")
                )
            })?;
            let command_status = output.status.code().unwrap_or(1);
            provider_commands.push(ProviderCommandReceipt {
                language_id: prepared.provider.language_id.clone(),
                provider_id: prepared.provider.provider_id.clone(),
                argv: provider_argv,
                exit_code: command_status,
                stdout_bytes: ByteCount::from_len(output.receipt.stdout_bytes),
                stderr_bytes: ByteCount::from_len(output.receipt.stderr_bytes),
                stdout_sha256: output.receipt.stdout_sha256.clone(),
                stderr_sha256: output.receipt.stderr_sha256.clone(),
                stdout_truncated: output.receipt.stdout_truncated,
                stderr_truncated: output.receipt.stderr_truncated,
                timed_out: output.receipt.timed_out,
                elapsed_ms: ElapsedMillis::from_duration(output.receipt.elapsed),
            });
            stdout.extend_from_slice(output.stdout.as_ref());
            stderr.extend_from_slice(output.stderr.as_ref());
            if command_status != 0 {
                status_code = command_status;
                break;
            }
        }

        Ok((
            provider,
            stdout.freeze(),
            stderr.freeze(),
            status_code,
            provider_commands,
            ElapsedMillis::from_duration(started_all.elapsed()),
        ))
    }

    fn protocol_renderer_env() -> BTreeMap<String, String> {
        let mut envs = BTreeMap::new();
        if std::env::var_os(SEMANTIC_AGENT_PROTOCOL_BIN_ENV).is_some() {
            return envs;
        }
        if let Ok(current_exe) = std::env::current_exe() {
            envs.insert(
                SEMANTIC_AGENT_PROTOCOL_BIN_ENV.to_string(),
                current_exe.to_string_lossy().to_string(),
            );
        }
        envs
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

fn provider_process_cwd(project_root: &std::path::Path) -> Result<PathBuf, String> {
    if project_root.is_absolute() {
        return Ok(project_root.to_path_buf());
    }
    project_root.canonicalize().map_err(|error| {
        format!(
            "provider project root `{}` is not executable: {error}",
            project_root.display()
        )
    })
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
