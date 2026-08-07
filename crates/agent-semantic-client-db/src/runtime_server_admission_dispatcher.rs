use std::{future::Future, pin::Pin, sync::Arc};

type AdmissionBuildFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

fn generation_data_plane_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        let available = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        let mut worker_threads = 1usize;
        while worker_threads.saturating_mul(worker_threads) < available {
            worker_threads = worker_threads.saturating_add(1);
        }
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .thread_name("asp-generation-data-plane")
            .enable_all()
            .build()
            .expect("build adaptive Runtime generation data-plane")
    })
}

enum AdmissionDispatcherCommand {
    Warmup,
    Spawn(AdmissionBuildFuture),
    Shutdown(tokio::sync::oneshot::Sender<usize>),
}

#[derive(Clone)]
pub(crate) struct RuntimeServerAdmissionDispatcher {
    sender: tokio::sync::mpsc::UnboundedSender<AdmissionDispatcherCommand>,
    shutting_down: Arc<std::sync::atomic::AtomicBool>,
}

impl RuntimeServerAdmissionDispatcher {
    pub(crate) fn new() -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let shutting_down = Arc::new(std::sync::atomic::AtomicBool::new(false));
        generation_data_plane_runtime().spawn(async move {
            let mut builds = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    command = receiver.recv() => match command {
                            Some(AdmissionDispatcherCommand::Warmup) => {}
                            Some(AdmissionDispatcherCommand::Spawn(build)) => {
                            builds.spawn(build);
                        }
                        Some(AdmissionDispatcherCommand::Shutdown(receipt)) => {
                            receiver.close();
                            builds.abort_all();
                            let task_count = builds.len();
                            while builds.join_next().await.is_some() {}
                            let _ = receipt.send(task_count);
                            break;
                        }
                        None => {
                            builds.abort_all();
                            while builds.join_next().await.is_some() {}
                            break;
                        }
                    },
                    Some(_) = builds.join_next(), if !builds.is_empty() => {}
                }
            }
        });
        let dispatcher = Self {
            sender,
            shutting_down,
        };
        let _ = dispatcher.sender.send(AdmissionDispatcherCommand::Warmup);
        dispatcher
    }

    pub(crate) fn spawn(&self, build: AdmissionBuildFuture) -> Result<(), AdmissionBuildFuture> {
        if self
            .shutting_down
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(build);
        }
        self.sender
            .send(AdmissionDispatcherCommand::Spawn(build))
            .map_err(|error| match error.0 {
                AdmissionDispatcherCommand::Spawn(build) => build,
                AdmissionDispatcherCommand::Warmup => {
                    unreachable!("spawn cannot return a warmup command")
                }
                AdmissionDispatcherCommand::Shutdown(_) => {
                    unreachable!("spawn cannot return a shutdown command")
                }
            })
    }

    pub(crate) async fn shutdown(&self) -> Result<usize, String> {
        if self
            .shutting_down
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Ok(0);
        }
        let (receipt, completed) = tokio::sync::oneshot::channel();
        self.sender
            .send(AdmissionDispatcherCommand::Shutdown(receipt))
            .map_err(|_| "workspace generation admission dispatcher is unavailable".to_owned())?;
        completed.await.map_err(|_| {
            "workspace generation admission dispatcher closed without a drain receipt".to_owned()
        })
    }
}
