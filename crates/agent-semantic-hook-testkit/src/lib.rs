use std::{
    future::Future,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Semaphore,
    task::JoinSet,
};

pub mod hook_scenarios;

pub const DEFAULT_HOOK_TIMEOUT: Duration = Duration::from_secs(2);
pub const DEFAULT_SCENARIO_CONCURRENCY: usize = 8;

#[derive(Clone, Debug)]
pub struct HookProcessSpec {
    pub executable: PathBuf,
    pub current_dir: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
}

impl HookProcessSpec {
    pub fn new(executable: impl Into<PathBuf>, current_dir: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            current_dir: current_dir.into(),
            args: Vec::new(),
            env: Vec::new(),
            timeout: DEFAULT_HOOK_TIMEOUT,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookScenario {
    pub scenario_id: String,
    pub payload: Value,
    pub expected_rule_id: Option<String>,
    pub forbidden_rule_id: Option<String>,
    pub expected_provider_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HookScenarioReceipt {
    pub scenario_id: String,
    pub decision: Value,
    pub elapsed: Duration,
}

#[derive(Debug)]
pub enum HookTestKitError {
    Spawn(std::io::Error),
    Io(std::io::Error),
    Timeout {
        timeout: Duration,
    },
    Exit {
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    Decode(serde_json::Error),
    Encode(serde_json::Error),
    Scenario {
        scenario_id: String,
        message: String,
    },
    Join(tokio::task::JoinError),
}

impl std::fmt::Display for HookTestKitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(formatter, "spawn Hook process: {error}"),
            Self::Io(error) => write!(formatter, "Hook process I/O: {error}"),
            Self::Timeout { timeout } => write!(
                formatter,
                "Hook process exceeded {}ms and was killed and reaped",
                timeout.as_millis()
            ),
            Self::Exit {
                status,
                stdout,
                stderr,
            } => write!(
                formatter,
                "Hook process failed with {status}: stdout={} stderr={}",
                String::from_utf8_lossy(stdout),
                String::from_utf8_lossy(stderr)
            ),
            Self::Decode(error) => write!(formatter, "decode Hook decision: {error}"),
            Self::Encode(error) => write!(formatter, "encode Hook decision: {error}"),
            Self::Scenario {
                scenario_id,
                message,
            } => {
                write!(formatter, "Hook scenario {scenario_id}: {message}")
            }
            Self::Join(error) => write!(formatter, "join Hook scenario task: {error}"),
        }
    }
}

impl std::error::Error for HookTestKitError {}

pub fn classify_hook_scenario(
    registry: &agent_semantic_hook::HookRuntime,
    config: &agent_semantic_hook::ClientHookConfig,
    platform: &str,
    event: &str,
    payload: &Value,
) -> Result<Value, HookTestKitError> {
    let decision = agent_semantic_hook::classify_hook_with_config(
        agent_semantic_hook::HookClassificationRequest {
            registry,
            config,
            platform,
            event,
            payload,
        },
    );
    serde_json::to_value(decision).map_err(HookTestKitError::Encode)
}

pub async fn run_hook_process(
    spec: &HookProcessSpec,
    payload: &Value,
) -> Result<HookScenarioReceipt, HookTestKitError> {
    let started = tokio::time::Instant::now();
    let mut command = tokio::process::Command::new(&spec.executable);
    command
        .current_dir(&spec.current_dir)
        .args(&spec.args)
        .envs(spec.env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(HookTestKitError::Spawn)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| HookTestKitError::Io(std::io::Error::other("Hook stdin was not piped")))?;
    stdin
        .write_all(payload.to_string().as_bytes())
        .await
        .map_err(HookTestKitError::Io)?;
    stdin.shutdown().await.map_err(HookTestKitError::Io)?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| HookTestKitError::Io(std::io::Error::other("Hook stdout was not piped")))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| HookTestKitError::Io(std::io::Error::other("Hook stderr was not piped")))?;
    let completed = tokio::time::timeout(spec.timeout, async {
        let read_stdout = async {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).await.map(|_| bytes)
        };
        let read_stderr = async {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.map(|_| bytes)
        };
        let (status, stdout, stderr) = tokio::join!(child.wait(), read_stdout, read_stderr);
        (status, stdout, stderr)
    })
    .await;
    let (status, stdout, stderr) = match completed {
        Ok((status, stdout, stderr)) => (
            status.map_err(HookTestKitError::Io)?,
            stdout.map_err(HookTestKitError::Io)?,
            stderr.map_err(HookTestKitError::Io)?,
        ),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(HookTestKitError::Timeout {
                timeout: spec.timeout,
            });
        }
    };
    if !status.success() {
        return Err(HookTestKitError::Exit {
            status,
            stdout,
            stderr,
        });
    }
    let decision = serde_json::from_slice(&stdout).map_err(HookTestKitError::Decode)?;
    Ok(HookScenarioReceipt {
        scenario_id: String::new(),
        decision,
        elapsed: started.elapsed(),
    })
}

pub async fn run_process_scenarios(
    spec: HookProcessSpec,
    scenarios: Vec<HookScenario>,
    concurrency: usize,
) -> Result<Vec<HookScenarioReceipt>, HookTestKitError> {
    let permits = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut tasks = JoinSet::new();
    let scenario_count = scenarios.len();
    for (index, scenario) in scenarios.into_iter().enumerate() {
        let spec = spec.clone();
        let permits = Arc::clone(&permits);
        tasks.spawn(async move {
            let _permit =
                permits
                    .acquire_owned()
                    .await
                    .map_err(|error| HookTestKitError::Scenario {
                        scenario_id: scenario.scenario_id.clone(),
                        message: error.to_string(),
                    })?;
            let mut receipt = run_hook_process(&spec, &scenario.payload).await?;
            receipt.scenario_id.clone_from(&scenario.scenario_id);
            assert_scenario(&scenario, &receipt.decision)?;
            Ok::<_, HookTestKitError>((index, receipt))
        });
    }
    let mut receipts = vec![None; scenario_count];
    while let Some(result) = tasks.join_next().await {
        let (index, receipt) = result.map_err(HookTestKitError::Join)??;
        receipts[index] = Some(receipt);
    }
    receipts
        .into_iter()
        .enumerate()
        .map(|(index, receipt)| {
            receipt.ok_or_else(|| HookTestKitError::Scenario {
                scenario_id: format!("index-{index}"),
                message: "scenario task produced no receipt".to_owned(),
            })
        })
        .collect()
}

pub async fn run_scenarios_with<F, Fut>(
    scenarios: Vec<HookScenario>,
    concurrency: usize,
    timeout: Duration,
    execute: F,
) -> Result<Vec<HookScenarioReceipt>, HookTestKitError>
where
    F: Fn(Value) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<Value, String>> + Send + 'static,
{
    let permits = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut tasks = JoinSet::new();
    let scenario_count = scenarios.len();
    for (index, scenario) in scenarios.into_iter().enumerate() {
        let permits = Arc::clone(&permits);
        let execute = execute.clone();
        tasks.spawn(async move {
            let _permit =
                permits
                    .acquire_owned()
                    .await
                    .map_err(|error| HookTestKitError::Scenario {
                        scenario_id: scenario.scenario_id.clone(),
                        message: error.to_string(),
                    })?;
            let started = tokio::time::Instant::now();
            let decision = tokio::time::timeout(timeout, execute(scenario.payload.clone()))
                .await
                .map_err(|_| HookTestKitError::Timeout { timeout })?
                .map_err(|message| HookTestKitError::Scenario {
                    scenario_id: scenario.scenario_id.clone(),
                    message,
                })?;
            assert_scenario(&scenario, &decision)?;
            Ok::<_, HookTestKitError>((
                index,
                HookScenarioReceipt {
                    scenario_id: scenario.scenario_id,
                    decision,
                    elapsed: started.elapsed(),
                },
            ))
        });
    }
    let mut receipts = vec![None; scenario_count];
    while let Some(result) = tasks.join_next().await {
        let (index, receipt) = result.map_err(HookTestKitError::Join)??;
        receipts[index] = Some(receipt);
    }
    receipts
        .into_iter()
        .enumerate()
        .map(|(index, receipt)| {
            receipt.ok_or_else(|| HookTestKitError::Scenario {
                scenario_id: format!("index-{index}"),
                message: "scenario task produced no receipt".to_owned(),
            })
        })
        .collect()
}

fn assert_scenario(scenario: &HookScenario, decision: &Value) -> Result<(), HookTestKitError> {
    let rule_id = decision
        .get("fields")
        .and_then(|fields| fields.get("configRuleId"))
        .and_then(Value::as_str);
    if rule_id != scenario.expected_rule_id.as_deref() {
        return Err(HookTestKitError::Scenario {
            scenario_id: scenario.scenario_id.clone(),
            message: format!(
                "expected rule {:?}, got {rule_id:?}",
                scenario.expected_rule_id
            ),
        });
    }
    if rule_id == scenario.forbidden_rule_id.as_deref() {
        return Err(HookTestKitError::Scenario {
            scenario_id: scenario.scenario_id.clone(),
            message: format!("forbidden rule {rule_id:?} won"),
        });
    }
    let provider_id = decision
        .get("routes")
        .and_then(Value::as_array)
        .and_then(|routes| routes.first())
        .and_then(|route| route.get("providerId"))
        .and_then(Value::as_str);
    if provider_id != scenario.expected_provider_id.as_deref() {
        return Err(HookTestKitError::Scenario {
            scenario_id: scenario.scenario_id.clone(),
            message: format!(
                "expected provider {:?}, got {provider_id:?}",
                scenario.expected_provider_id
            ),
        });
    }
    Ok(())
}
