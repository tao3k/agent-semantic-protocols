use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::net::UnixStream;
use tokio::sync::{Mutex, OnceCell, RwLock};

use super::frame::{read_frame, write_frame};
use super::{
    RuntimeServerControlReceipt, RuntimeServerControlRequest, RuntimeServerEndpoint,
    runtime_server_connection_pool_capacity,
};

static RUNTIME_SERVER_CONNECTION_POOL: OnceCell<
    RwLock<Option<(String, Arc<RuntimeServerConnectionPool>)>>,
> = OnceCell::const_new();

pub(super) struct RuntimeServerConnectionPool {
    endpoint: RuntimeServerEndpoint,
    next_lane: AtomicUsize,
    lanes: RwLock<Vec<Arc<RuntimeServerLane>>>,
    max_lanes: usize,
}

struct RuntimeServerLane {
    stream: Mutex<Option<UnixStream>>,
    in_flight: AtomicUsize,
}

impl RuntimeServerConnectionPool {
    fn new(endpoint: &RuntimeServerEndpoint) -> Self {
        Self {
            endpoint: endpoint.clone(),
            next_lane: AtomicUsize::new(0),
            lanes: RwLock::new(Vec::new()),
            max_lanes: runtime_server_connection_pool_capacity(),
        }
    }

    async fn reserve_lane(&self) -> Arc<RuntimeServerLane> {
        {
            let lanes = self.lanes.read().await;
            if let Some(lane) = self.try_reserve_idle_lane(&lanes) {
                return lane;
            }
            if lanes.len() >= self.max_lanes {
                return self.reserve_least_loaded_lane(&lanes);
            }
        }

        let mut lanes = self.lanes.write().await;
        if let Some(lane) = self.try_reserve_idle_lane(&lanes) {
            return lane;
        }
        if lanes.len() < self.max_lanes {
            let first_new_lane = lanes.len();
            let growth = lanes
                .len()
                .min(self.max_lanes.saturating_sub(lanes.len()))
                .max(1);
            lanes.extend((0..growth).map(|_| {
                Arc::new(RuntimeServerLane {
                    stream: Mutex::new(None),
                    in_flight: AtomicUsize::new(0),
                })
            }));
            let lane = Arc::clone(&lanes[first_new_lane]);
            lane.in_flight.store(1, Ordering::Release);
            return lane;
        }
        self.reserve_least_loaded_lane(&lanes)
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
        let lane = self.reserve_lane().await;
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
        endpoint.socket_path, endpoint.owner_epoch, endpoint.runtime_artifact_digest
    );
    let state = RUNTIME_SERVER_CONNECTION_POOL
        .get_or_init(|| async { RwLock::new(None) })
        .await;
    {
        let guard = state.read().await;
        if let Some((active_key, pool)) = guard.as_ref()
            && active_key == &key
        {
            return Arc::clone(pool);
        }
    }
    let mut guard = state.write().await;
    if let Some((active_key, pool)) = guard.as_ref()
        && active_key == &key
    {
        return Arc::clone(pool);
    }
    let pool = Arc::new(RuntimeServerConnectionPool::new(endpoint));
    *guard = Some((key, Arc::clone(&pool)));
    pool
}

async fn exchange_runtime_server_request(
    endpoint: &RuntimeServerEndpoint,
    stream: &mut Option<UnixStream>,
    request: &RuntimeServerControlRequest,
) -> Result<RuntimeServerControlReceipt, String> {
    if stream.is_none() {
        *stream = Some(
            UnixStream::connect(&endpoint.socket_path)
                .await
                .map_err(|error| format!("failed to connect Runtime Server endpoint: {error}"))?,
        );
    }
    let active = stream
        .as_mut()
        .expect("Runtime Server lane stream initialized above");
    let result = async {
        write_frame(active, &[request]).await?;
        let mut receipts = read_frame::<_, Vec<RuntimeServerControlReceipt>>(active).await?;
        if receipts.len() != 1 {
            return Err(format!(
                "Runtime Server returned {} receipts for one control request",
                receipts.len()
            ));
        }
        Ok(receipts.remove(0))
    }
    .await;
    if result.is_err() {
        *stream = None;
    }
    result
}
