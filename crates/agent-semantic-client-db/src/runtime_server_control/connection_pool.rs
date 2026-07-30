use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::net::UnixStream;
use tokio::sync::{OnceCell, RwLock, mpsc, oneshot};

use super::{
    RuntimeServerControlReceipt, RuntimeServerControlRequest, RuntimeServerEndpoint,
    runtime_server_connection_pool_capacity, runtime_server_connection_pool_size,
};
use super::frame::{read_frame, write_frame};

const RUNTIME_SERVER_LANE_QUEUE_CAPACITY: usize = 64;

static RUNTIME_SERVER_CONNECTION_POOL: OnceCell<
    RwLock<Option<(String, Arc<RuntimeServerConnectionPool>)>>,
> = OnceCell::const_new();

pub(super) struct RuntimeServerConnectionPool {
    endpoint: RuntimeServerEndpoint,
    next_lane: AtomicUsize,
    lanes: RwLock<Vec<RuntimeServerLane>>,
    max_lanes: usize,
}

struct RuntimeServerLane {
    sender: mpsc::Sender<RuntimeServerExchange>,
    in_flight: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for RuntimeServerConnectionPool {
    fn drop(&mut self) {
        for lane in self.lanes.get_mut() {
            lane.task.abort();
        }
    }
}

struct RuntimeServerExchange {
    request: RuntimeServerControlRequest,
    response: oneshot::Sender<Result<RuntimeServerControlReceipt, String>>,
}

impl RuntimeServerConnectionPool {
    fn new(endpoint: &RuntimeServerEndpoint) -> Self {
        let lane_count = runtime_server_connection_pool_size();
        let lanes = (0..lane_count)
            .map(|_| spawn_runtime_server_lane(endpoint))
            .collect();
        Self {
            endpoint: endpoint.clone(),
            next_lane: AtomicUsize::new(0),
            lanes: RwLock::new(lanes),
            max_lanes: runtime_server_connection_pool_capacity(),
        }
    }

    async fn reserve_lane(
        &self,
    ) -> (mpsc::Sender<RuntimeServerExchange>, Arc<AtomicUsize>) {
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
            lanes.extend((0..growth).map(|_| spawn_runtime_server_lane(&self.endpoint)));
            let lane = &lanes[first_new_lane];
            lane.in_flight.store(1, Ordering::Release);
            return (lane.sender.clone(), Arc::clone(&lane.in_flight));
        }
        self.reserve_least_loaded_lane(&lanes)
    }

    fn try_reserve_idle_lane(
        &self,
        lanes: &[RuntimeServerLane],
    ) -> Option<(mpsc::Sender<RuntimeServerExchange>, Arc<AtomicUsize>)> {
        let start = self.next_lane.fetch_add(1, Ordering::Relaxed);
        for offset in 0..lanes.len() {
            let lane = &lanes[(start + offset) % lanes.len()];
            if lane
                .in_flight
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Some((lane.sender.clone(), Arc::clone(&lane.in_flight)));
            }
        }
        None
    }

    fn reserve_least_loaded_lane(
        &self,
        lanes: &[RuntimeServerLane],
    ) -> (mpsc::Sender<RuntimeServerExchange>, Arc<AtomicUsize>) {
        let lane = lanes
            .iter()
            .min_by_key(|lane| lane.in_flight.load(Ordering::Relaxed))
            .expect("Runtime Server connection pool always has a base lane");
        lane.in_flight.fetch_add(1, Ordering::Acquire);
        (lane.sender.clone(), Arc::clone(&lane.in_flight))
    }

    pub(super) async fn exchange(
        &self,
        request: RuntimeServerControlRequest,
    ) -> Result<RuntimeServerControlReceipt, String> {
        let (lane, in_flight) = self.reserve_lane().await;
        let (response, receipt) = oneshot::channel();
        let sent = lane
            .send(RuntimeServerExchange { request, response })
            .await
            .map_err(|_| "Runtime Server connection lane is unavailable".to_owned());
        if let Err(error) = sent {
            in_flight.fetch_sub(1, Ordering::Release);
            return Err(error);
        }
        let result = receipt
            .await
            .map_err(|_| "Runtime Server connection lane closed without a receipt".to_owned())?;
        in_flight.fetch_sub(1, Ordering::Release);
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

fn spawn_runtime_server_lane(endpoint: &RuntimeServerEndpoint) -> RuntimeServerLane {
    let (sender, receiver) = mpsc::channel(RUNTIME_SERVER_LANE_QUEUE_CAPACITY);
    let task = tokio::spawn(run_runtime_server_lane(endpoint.clone(), receiver));
    RuntimeServerLane {
        sender,
        in_flight: Arc::new(AtomicUsize::new(0)),
        task,
    }
}

async fn run_runtime_server_lane(
    endpoint: RuntimeServerEndpoint,
    mut exchanges: mpsc::Receiver<RuntimeServerExchange>,
) {
    let mut stream = None;
    while let Some(first) = exchanges.recv().await {
        let batch = collect_runtime_server_batch(first, &mut exchanges);
        let result = exchange_runtime_server_batch(&endpoint, &mut stream, &batch).await;
        complete_runtime_server_batch(batch, result);
    }
}

fn collect_runtime_server_batch(
    first: RuntimeServerExchange,
    exchanges: &mut mpsc::Receiver<RuntimeServerExchange>,
) -> Vec<RuntimeServerExchange> {
    let mut batch = vec![first];
    while let Ok(exchange) = exchanges.try_recv() {
        batch.push(exchange);
    }
    batch
}

async fn exchange_runtime_server_batch(
    endpoint: &RuntimeServerEndpoint,
    stream: &mut Option<UnixStream>,
    batch: &[RuntimeServerExchange],
) -> Result<Vec<RuntimeServerControlReceipt>, String> {
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
    let requests = batch
        .iter()
        .map(|exchange| &exchange.request)
        .collect::<Vec<_>>();
    let result = async {
        write_frame(active, &requests).await?;
        read_frame::<_, Vec<RuntimeServerControlReceipt>>(active).await
    }
    .await;
    if result.is_err() {
        *stream = None;
    }
    result
}

fn complete_runtime_server_batch(
    batch: Vec<RuntimeServerExchange>,
    result: Result<Vec<RuntimeServerControlReceipt>, String>,
) {
    match result {
        Ok(receipts) => {
            for (exchange, receipt) in batch.into_iter().zip(receipts) {
                let _ = exchange.response.send(Ok(receipt));
            }
        }
        Err(error) => {
            for exchange in batch {
                let _ = exchange.response.send(Err(error.clone()));
            }
        }
    }
}
