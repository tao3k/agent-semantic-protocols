use std::{future::Future, pin::Pin};

type AdmissionBuildFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub(crate) fn spawn_admission_authority(
    authority: impl Future<Output = ()> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    match tokio::runtime::Handle::try_current() {
        Ok(runtime) => runtime.spawn(authority),
        Err(_) => generation_data_plane_runtime().spawn(authority),
    }
}

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
    Track(tokio::task::JoinHandle<()>),
    Shutdown(tokio::sync::oneshot::Sender<usize>),
}

#[derive(Clone)]
pub(crate) struct RuntimeServerAdmissionDispatcher {
    sender: tokio::sync::mpsc::UnboundedSender<AdmissionDispatcherCommand>,
}

impl RuntimeServerAdmissionDispatcher {
    pub(crate) fn new() -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        generation_data_plane_runtime().spawn(async move {
            let mut builds = tokio::task::JoinSet::new();
            let mut tracked = Vec::<tokio::task::JoinHandle<()>>::new();
            loop {
                tokio::select! {
                    command = receiver.recv() => match command {
                            Some(AdmissionDispatcherCommand::Warmup) => {}
                            Some(AdmissionDispatcherCommand::Spawn(build)) => {
                            builds.spawn(build);
                        }
                        Some(AdmissionDispatcherCommand::Track(task)) => {
                            tracked.retain(|task| !task.is_finished());
                            tracked.push(task);
                        }
                        Some(AdmissionDispatcherCommand::Shutdown(receipt)) => {
                            receiver.close();
                            builds.abort_all();
                            for task in &tracked {
                                task.abort();
                            }
                            let task_count = builds.len() + tracked.len();
                            while builds.join_next().await.is_some() {}
                            for task in tracked.drain(..) {
                                let _ = task.await;
                            }
                            let _ = receipt.send(task_count);
                            break;
                        }
                        None => {
                            builds.abort_all();
                            for task in &tracked {
                                task.abort();
                            }
                            while builds.join_next().await.is_some() {}
                            for task in tracked.drain(..) {
                                let _ = task.await;
                            }
                            break;
                        }
                    },
                    Some(_) = builds.join_next(), if !builds.is_empty() => {}
                }
            }
        });
        let dispatcher = Self { sender };
        let _ = dispatcher.sender.send(AdmissionDispatcherCommand::Warmup);
        dispatcher
    }

    pub(crate) fn spawn(&self, build: AdmissionBuildFuture) -> Result<(), AdmissionBuildFuture> {
        self.sender
            .send(AdmissionDispatcherCommand::Spawn(build))
            .map_err(|error| match error.0 {
                AdmissionDispatcherCommand::Spawn(build) => build,
                AdmissionDispatcherCommand::Warmup => {
                    unreachable!("spawn cannot return a warmup command")
                }
                AdmissionDispatcherCommand::Track(_) => {
                    unreachable!("spawn cannot return a tracked task")
                }
                AdmissionDispatcherCommand::Shutdown(_) => {
                    unreachable!("spawn cannot return a shutdown command")
                }
            })
    }

    pub(crate) fn track(&self, task: tokio::task::JoinHandle<()>) {
        if let Err(error) = self.sender.send(AdmissionDispatcherCommand::Track(task)) {
            if let AdmissionDispatcherCommand::Track(task) = error.0 {
                task.abort();
            }
        }
    }

    pub(crate) async fn shutdown(&self) -> Result<usize, String> {
        let (receipt, completed) = tokio::sync::oneshot::channel();
        if self
            .sender
            .send(AdmissionDispatcherCommand::Shutdown(receipt))
            .is_err()
        {
            return Ok(0);
        }
        Ok(completed.await.unwrap_or(0))
    }
}
