use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::net::UnixStream;
use tokio::sync::{Mutex, OnceCell, RwLock};

use super::frame::{read_frame, write_frame};
use super::{
    RuntimeServerControlReceipt, RuntimeServerControlRequest, RuntimeServerEndpoint,
    runtime_server_connection_pool_capacity,
};

static RUNTIME_SERVER_CONNECTION_POOLS: OnceCell<
    RwLock<HashMap<String, Arc<RuntimeServerConnectionPool>>>,
> = OnceCell::const_new();

const RUNTIME_SERVER_CONTROL_EXCHANGE_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(650);

pub(super) struct RuntimeServerConnectionPool {
    endpoint: RuntimeServerEndpoint,
    next_lane: AtomicUsize,
    lanes: Vec<Arc<RuntimeServerLane>>,
}

struct RuntimeServerLane {
    stream: Mutex<Option<UnixStream>>,
    in_flight: AtomicUsize,
}

impl RuntimeServerConnectionPool {
    fn new(endpoint: &RuntimeServerEndpoint) -> Self {
        let lane_count = runtime_server_connection_pool_capacity();
        Self {
            endpoint: endpoint.clone(),
            next_lane: AtomicUsize::new(0),
            lanes: (0..lane_count)
                .map(|_| {
                    Arc::new(RuntimeServerLane {
                        stream: Mutex::new(None),
                        in_flight: AtomicUsize::new(0),
                    })
                })
                .collect(),
        }
    }

    fn reserve_lane(&self) -> Arc<RuntimeServerLane> {
        if let Some(lane) = self.try_reserve_idle_lane(&self.lanes) {
            return lane;
        }
        self.reserve_least_loaded_lane(&self.lanes)
    }

    fn try_reserve_idle_lane(
        &self,
        lanes: &[Arc<RuntimeServerLane>],
    ) -> Option<Arc<RuntimeServerLane>> {
        let start = self.next_lane.fetch_add(1, Ordering::Relaxed);
        for offset in 0..lanes.len() {
            let lane = &lanes[(start + offset) % lanes.len()];
            if lane
                .in_flight
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Some(Arc::clone(lane));
            }
        }
        None
    }

    fn reserve_least_loaded_lane(
        &self,
        lanes: &[Arc<RuntimeServerLane>],
    ) -> Arc<RuntimeServerLane> {
        let lane = lanes
            .iter()
            .min_by_key(|lane| lane.in_flight.load(Ordering::Relaxed))
            .expect("Runtime Server connection pool always has a base lane");
        lane.in_flight.fetch_add(1, Ordering::Acquire);
        Arc::clone(lane)
    }

    pub(super) async fn exchange(
        &self,
        request: RuntimeServerControlRequest,
    ) -> Result<RuntimeServerControlReceipt, String> {
        let lane = self.reserve_lane();
        let mut stream = lane.stream.lock().await;
        let result = exchange_runtime_server_request(&self.endpoint, &mut stream, &request).await;
        lane.in_flight.fetch_sub(1, Ordering::Release);
        result
    }
}

pub(super) async fn connection_pool(
    endpoint: &RuntimeServerEndpoint,
) -> Arc<RuntimeServerConnectionPool> {
    let key = format!(
        "{}\0{}\0{}",
        endpoint.socket_path,
        endpoint.owner_epoch,
        endpoint.runtime_binary_identity.content_digest()
    );
    let pools = RUNTIME_SERVER_CONNECTION_POOLS
        .get_or_init(|| async { RwLock::new(HashMap::new()) })
        .await;
    {
        let guard = pools.read().await;
        if let Some(pool) = guard.get(&key) {
            return Arc::clone(pool);
        }
    }
    let mut guard = pools.write().await;
    if let Some(pool) = guard.get(&key) {
        return Arc::clone(pool);
    }
    guard.retain(|_, pool| pool.endpoint.socket_path != endpoint.socket_path);
    let pool = Arc::new(RuntimeServerConnectionPool::new(endpoint));
    guard.insert(key, Arc::clone(&pool));
    pool
}

async fn exchange_runtime_server_request(
    endpoint: &RuntimeServerEndpoint,
    stream: &mut Option<UnixStream>,
    request: &RuntimeServerControlRequest,
) -> Result<RuntimeServerControlReceipt, String> {
    exchange_runtime_server_request_with_budget(
        endpoint,
        stream,
        request,
        RUNTIME_SERVER_CONTROL_EXCHANGE_BUDGET,
    )
    .await
}

async fn exchange_runtime_server_request_with_budget(
    endpoint: &RuntimeServerEndpoint,
    stream: &mut Option<UnixStream>,
    request: &RuntimeServerControlRequest,
    budget: std::time::Duration,
) -> Result<RuntimeServerControlReceipt, String> {
    discard_closed_or_dirty_control_stream(stream);
    if stream.is_none() {
        let connected = UnixStream::connect(&endpoint.socket_path)
            .await
            .map_err(|error| format!("failed to connect Runtime Server endpoint: {error}"))?;
        super::validate_runtime_server_peer_fd(std::os::fd::AsRawFd::as_raw_fd(&connected))?;
        *stream = Some(connected);
    }
    let active = stream
        .as_mut()
        .expect("Runtime Server lane stream initialized above");
    let started = std::time::Instant::now();
    let result = match tokio::time::timeout(budget, async {
        write_frame(active, &[request]).await?;
        let mut receipts = read_frame::<_, Vec<RuntimeServerControlReceipt>>(active).await?;
        if receipts.len() != 1 {
            return Err(format!(
                "Runtime Server returned {} receipts for one control request",
                receipts.len()
            ));
        }
        Ok(receipts.remove(0))
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-control-wall-failure",
            "schemaVersion": "1",
            "state": "unavailable",
            "stage": "runtime-server-control-exchange",
            "reasonKind": "runtime-server-control-exchange-budget-exceeded",
            "executionBudgetMicros": budget.as_micros(),
            "elapsedMicros": started.elapsed().as_micros(),
            "retryAfterMs": 250,
        })
        .to_string()),
    };
    if result.is_err() {
        *stream = None;
    }
    result
}

fn discard_closed_or_dirty_control_stream(stream: &mut Option<UnixStream>) {
    let Some(active) = stream.as_mut() else {
        return;
    };
    let mut probe = [0_u8; 1];
    match active.try_read(&mut probe) {
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
        // A pooled lane is only idle after its complete response frame was
        // consumed. EOF, an error, or unexpected unread bytes all mean this
        // stream cannot safely carry the next request generation.
        Ok(_) | Err(_) => *stream = None,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_control_connection_pool.rs"]
mod tests;
