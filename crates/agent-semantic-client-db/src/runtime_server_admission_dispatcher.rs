//! Runtime-owned dispatcher for generation admission and mutation futures.

use std::{future::Future, pin::Pin, sync::Arc};

type AdmissionBuildFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

enum AdmissionDispatcherCommand {
    Warmup,
    Spawn(AdmissionBuildFuture),
    Shutdown(tokio::sync::oneshot::Sender<usize>),
}

#[derive(Clone)]
pub(crate) struct RuntimeServerAdmissionDispatcher {
    sender: tokio::sync::mpsc::Sender<AdmissionDispatcherCommand>,
    task:
        Arc<tokio::sync::Mutex<Option<crate::runtime_server_runtime::RuntimeServerOwnedTask<()>>>>,
    task_scope: crate::runtime_server_runtime::RuntimeServerTaskScope,
}

impl RuntimeServerAdmissionDispatcher {
    pub(crate) fn new() -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
        let task_scope =
            crate::runtime_server_runtime::RuntimeServerTaskScope::new("generation-admission");
        let task = task_scope
            .spawn("generation-admission-dispatcher", async move {
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
            })
            .expect("new generation-admission task scope accepts its owner task");
        let dispatcher = Self {
            sender,
            task: Arc::new(tokio::sync::Mutex::new(Some(task))),
            task_scope,
        };
        let _ = dispatcher
            .sender
            .try_send(AdmissionDispatcherCommand::Warmup);
        dispatcher
    }

    pub(crate) fn spawn(&self, build: AdmissionBuildFuture) -> Result<(), AdmissionBuildFuture> {
        self.sender
            .try_send(AdmissionDispatcherCommand::Spawn(build))
            .map_err(|error| match error.into_inner() {
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
        let (receipt, completed) = tokio::sync::oneshot::channel();
        let task_count = if self
            .sender
            .send(AdmissionDispatcherCommand::Shutdown(receipt))
            .await
            .is_err()
        {
            0
        } else {
            completed.await.unwrap_or(0)
        };
        self.task_scope.begin_drain();
        if let Some(task) = self.task.lock().await.take() {
            task.join().await?;
        }
        self.task_scope.finish(0)?;
        Ok(task_count)
    }
}
