// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Engineering governance primitives for [`crate::AspForwardingGateway`]:
//! - Loopback per-session Bearer control-plane token ([`GatewaySessionToken`])
//! - Per-language circuit breaker ([`ProviderCircuitBreakerPolicy`])
//! - Structured health, lifecycle, and latency telemetry receipts ([`ProviderHealthReceipt`])

use std::collections::BTreeMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::{AstEngineError, AstLanguageId, AstProviderId};

static TOKEN_NONCE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Validated loopback per-session Bearer token protecting gateway control-plane
/// routes (`POST /v1/providers/register`, `POST /shutdown`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewaySessionToken(String);

impl GatewaySessionToken {
    /// Construct a validated non-empty control token.
    pub fn new(raw: impl Into<String>) -> Result<Self, AstEngineError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.len() < 16 {
            return Err(AstEngineError::InvalidIdentifier(
                "gateway control session token must be at least 16 characters",
            ));
        }
        Ok(Self(trimmed))
    }

    /// Generate a fresh high-entropy per-session token without external RNG crates.
    #[must_use]
    pub fn generate() -> Self {
        let pid = std::process::id();
        let seq = TOKEN_NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let now_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let addr_entropy = (&seq as *const u64) as usize;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"asp-gateway-session-token-v1:");
        hasher.update(&pid.to_le_bytes());
        hasher.update(&seq.to_le_bytes());
        hasher.update(&now_epoch.to_le_bytes());
        hasher.update(&addr_entropy.to_le_bytes());
        Self(hasher.finalize().to_hex().to_string())
    }

    /// Borrow the raw bearer token string (emitted once on bootstrap stdout pipe).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Constant-time verification of an HTTP `Authorization: Bearer <token>` header.
    #[must_use]
    pub fn verify_authorization_header(&self, header: Option<&str>) -> bool {
        let Some(raw) = header else {
            return false;
        };
        let Some(candidate) = raw
            .strip_prefix("Bearer ")
            .or_else(|| raw.strip_prefix("bearer "))
        else {
            return false;
        };
        constant_time_eq(self.0.as_bytes(), candidate.trim().as_bytes())
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Circuit-breaker policy governing per-language provider failure isolation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderCircuitBreakerPolicy {
    /// Number of consecutive provider errors before opening the circuit.
    pub failure_threshold: u32,
    /// Duration the circuit remains open before admitting a half-open probe.
    pub cooldown: Duration,
}

impl Default for ProviderCircuitBreakerPolicy {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            cooldown: Duration::from_secs(15),
        }
    }
}

/// Observable state of a language provider's circuit breaker.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Concrete runtime execution class for a provider.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRuntimeKind {
    AotBinary,
    NodeBundle,
    InterpreterScript,
    ExternalHttp,
    EmbeddedPort,
    DescriptorOnly,
}

/// Lifecycle activation mode of a provider within the gateway.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderSpawnMode {
    LazyOnDemand,
    Eager,
    External,
}

/// Live process readiness state of a supervised provider.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderLifecycleState {
    Dormant,
    Warm,
    Degraded,
}

/// Structured per-provider health, circuit, and latency receipt exposed on `GET /health`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealthReceipt {
    pub language_id: String,
    pub provider_id: String,
    pub runtime_kind: ProviderRuntimeKind,
    pub spawn_mode: ProviderSpawnMode,
    pub lifecycle_state: ProviderLifecycleState,
    pub circuit_state: CircuitBreakerState,
    pub consecutive_failures: u32,
    pub total_requests: u64,
    pub total_failures: u64,
    pub last_latency_micros: u64,
    pub avg_latency_micros: u64,
    pub idle_evictions: u64,
}

#[derive(Clone, Debug)]
struct ProviderTelemetryRecord {
    provider_id: AstProviderId,
    runtime_kind: ProviderRuntimeKind,
    spawn_mode: ProviderSpawnMode,
    lifecycle_state: ProviderLifecycleState,
    consecutive_failures: u32,
    opened_at: Option<Instant>,
    total_requests: u64,
    total_failures: u64,
    total_latency_micros: u128,
    last_latency_micros: u64,
    idle_evictions: u64,
}

impl ProviderTelemetryRecord {
    fn circuit_state(
        &self,
        now: Instant,
        policy: ProviderCircuitBreakerPolicy,
    ) -> CircuitBreakerState {
        let Some(opened_at) = self.opened_at else {
            return CircuitBreakerState::Closed;
        };
        if now.saturating_duration_since(opened_at) >= policy.cooldown {
            CircuitBreakerState::HalfOpen
        } else {
            CircuitBreakerState::Open
        }
    }

    fn to_receipt(
        &self,
        language_id: &AstLanguageId,
        now: Instant,
        policy: ProviderCircuitBreakerPolicy,
    ) -> ProviderHealthReceipt {
        let avg_latency_micros = if self.total_requests == 0 {
            0
        } else {
            u64::try_from(self.total_latency_micros / u128::from(self.total_requests))
                .unwrap_or(u64::MAX)
        };
        ProviderHealthReceipt {
            language_id: language_id.as_str().to_owned(),
            provider_id: self.provider_id.as_str().to_owned(),
            runtime_kind: self.runtime_kind,
            spawn_mode: self.spawn_mode,
            lifecycle_state: self.lifecycle_state,
            circuit_state: self.circuit_state(now, policy),
            consecutive_failures: self.consecutive_failures,
            total_requests: self.total_requests,
            total_failures: self.total_failures,
            last_latency_micros: self.last_latency_micros,
            avg_latency_micros,
            idle_evictions: self.idle_evictions,
        }
    }
}

/// Thread-safe per-language governance state tracking circuit breakers and latency telemetry.
#[derive(Debug)]
pub struct ProviderGovernanceTable {
    policy: ProviderCircuitBreakerPolicy,
    records: RwLock<BTreeMap<AstLanguageId, ProviderTelemetryRecord>>,
}

impl Default for ProviderGovernanceTable {
    fn default() -> Self {
        Self::new(ProviderCircuitBreakerPolicy::default())
    }
}

impl ProviderGovernanceTable {
    /// Construct a governance table with the given circuit breaker policy.
    #[must_use]
    pub fn new(policy: ProviderCircuitBreakerPolicy) -> Self {
        Self {
            policy,
            records: RwLock::new(BTreeMap::new()),
        }
    }

    /// Return the active circuit breaker policy.
    #[must_use]
    pub fn policy(&self) -> ProviderCircuitBreakerPolicy {
        self.policy
    }

    /// Register or refresh governance metadata for a language provider.
    pub fn register_provider(
        &self,
        language_id: AstLanguageId,
        provider_id: AstProviderId,
        runtime_kind: ProviderRuntimeKind,
        spawn_mode: ProviderSpawnMode,
        lifecycle_state: ProviderLifecycleState,
    ) -> Result<(), AstEngineError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| AstEngineError::ProviderError("governance table poisoned".to_owned()))?;
        records.insert(
            language_id,
            ProviderTelemetryRecord {
                provider_id,
                runtime_kind,
                spawn_mode,
                lifecycle_state,
                consecutive_failures: 0,
                opened_at: None,
                total_requests: 0,
                total_failures: 0,
                total_latency_micros: 0,
                last_latency_micros: 0,
                idle_evictions: 0,
            },
        );
        Ok(())
    }

    /// Check whether the per-language circuit breaker admits a new inspection call.
    pub fn check_circuit_admits(&self, language_id: &AstLanguageId) -> Result<(), String> {
        let records = self
            .records
            .read()
            .map_err(|_| "governance table poisoned".to_owned())?;
        let Some(rec) = records.get(language_id) else {
            return Ok(());
        };
        if rec.circuit_state(Instant::now(), self.policy) == CircuitBreakerState::Open {
            return Err(format!(
                "circuit breaker open for `{}` after {} consecutive failures (cooldown {}ms)",
                language_id.as_str(),
                rec.consecutive_failures,
                self.policy.cooldown.as_millis()
            ));
        }
        Ok(())
    }

    /// Record a successful provider inspection and close any half-open circuit.
    pub fn record_success(&self, language_id: &AstLanguageId, elapsed_micros: u64) {
        let Ok(mut records) = self.records.write() else {
            return;
        };
        if let Some(rec) = records.get_mut(language_id) {
            rec.consecutive_failures = 0;
            rec.opened_at = None;
            rec.lifecycle_state = ProviderLifecycleState::Warm;
            rec.total_requests = rec.total_requests.saturating_add(1);
            rec.last_latency_micros = elapsed_micros;
            rec.total_latency_micros = rec
                .total_latency_micros
                .saturating_add(u128::from(elapsed_micros));
        }
    }

    /// Record a provider error and open the circuit once `failure_threshold` is reached.
    pub fn record_failure(&self, language_id: &AstLanguageId, elapsed_micros: u64) {
        let Ok(mut records) = self.records.write() else {
            return;
        };
        if let Some(rec) = records.get_mut(language_id) {
            rec.consecutive_failures = rec.consecutive_failures.saturating_add(1);
            rec.total_requests = rec.total_requests.saturating_add(1);
            rec.total_failures = rec.total_failures.saturating_add(1);
            rec.last_latency_micros = elapsed_micros;
            rec.total_latency_micros = rec
                .total_latency_micros
                .saturating_add(u128::from(elapsed_micros));
            if rec.consecutive_failures >= self.policy.failure_threshold.max(1) {
                rec.opened_at = Some(Instant::now());
                rec.lifecycle_state = ProviderLifecycleState::Degraded;
            }
        }
    }

    /// Update a provider's live lifecycle state (e.g. `Warm` on lazy boot, `Dormant` on eviction).
    pub fn set_lifecycle_state(
        &self,
        language_id: &AstLanguageId,
        lifecycle_state: ProviderLifecycleState,
        counted_eviction: bool,
    ) {
        let Ok(mut records) = self.records.write() else {
            return;
        };
        if let Some(rec) = records.get_mut(language_id) {
            rec.lifecycle_state = lifecycle_state;
            if counted_eviction {
                rec.idle_evictions = rec.idle_evictions.saturating_add(1);
            }
        }
    }

    /// Snapshot structured health receipts for all known language providers.
    pub fn health_receipts(&self) -> Result<Vec<ProviderHealthReceipt>, AstEngineError> {
        let records = self
            .records
            .read()
            .map_err(|_| AstEngineError::ProviderError("governance table poisoned".to_owned()))?;
        let now = Instant::now();
        Ok(records
            .iter()
            .map(|(lang, rec)| rec.to_receipt(lang, now, self.policy))
            .collect())
    }
}
