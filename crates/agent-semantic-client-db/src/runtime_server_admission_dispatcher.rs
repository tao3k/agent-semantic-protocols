//! Runtime-owned dispatcher for generation admission and mutation futures.

use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

type AdmissionBuildFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type AdmissionBuildStartFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>>;

pub(crate) struct AdmissionBuildEnvelope {
    start: Option<AdmissionBuildStartFuture>,
    build: Option<AdmissionBuildFuture>,
    terminalize: Option<AdmissionBuildFuture>,
}

struct AdmissionBuildTerminalizers(HashMap<tokio::task::Id, AdmissionBuildFuture>);

impl AdmissionBuildTerminalizers {
    fn new() -> Self {
        Self(HashMap::new())
    }

    fn insert(&mut self, id: tokio::task::Id, terminalize: AdmissionBuildFuture) {
        self.0.insert(id, terminalize);
    }

    fn remove(&mut self, id: &tokio::task::Id) -> Option<AdmissionBuildFuture> {
        self.0.remove(id)
    }

    async fn terminalize_remaining(&mut self) {
        for (_, terminalize) in self.0.drain() {
            terminalize.await;
        }
    }
}

impl Drop for AdmissionBuildTerminalizers {
    fn drop(&mut self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        for (_, terminalize) in self.0.drain() {
            runtime.spawn(terminalize);
        }
    }
}

impl AdmissionBuildEnvelope {
    pub(crate) fn new(
        start: AdmissionBuildStartFuture,
        build: AdmissionBuildFuture,
        terminalize: AdmissionBuildFuture,
    ) -> Self {
        Self {
            start: Some(start),
            build: Some(build),
            terminalize: Some(terminalize),
        }
    }

    pub(crate) fn detached(build: AdmissionBuildFuture) -> Self {
        Self::new(Box::pin(async { Ok(()) }), build, Box::pin(async {}))
    }

    fn take_parts(
        &mut self,
    ) -> (
        AdmissionBuildStartFuture,
        AdmissionBuildFuture,
        AdmissionBuildFuture,
    ) {
        (
            self.start.take().expect("admission start is owned"),
            self.build.take().expect("admission build is owned"),
            self.terminalize
                .take()
                .expect("admission terminalizer is owned"),
        )
    }
}

impl Drop for AdmissionBuildEnvelope {
    fn drop(&mut self) {
        let Some(terminalize) = self.terminalize.take() else {
            return;
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(terminalize);
        }
    }
}

enum AdmissionDispatcherCommand {
    Warmup,
    Spawn(AdmissionBuildEnvelope),
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
                let mut terminalizers = AdmissionBuildTerminalizers::new();
                loop {
                    tokio::select! {
                        command = receiver.recv() => match command {
                            Some(AdmissionDispatcherCommand::Warmup) => {}
                            Some(AdmissionDispatcherCommand::Spawn(mut envelope)) => {
                                let (start, build, terminalize) = envelope.take_parts();
                                if start.await.is_ok() {
                                    let task = builds.spawn(build);
                                    terminalizers.insert(task.id(), terminalize);
                                } else {
                                    terminalize.await;
                                }
                            }
                            Some(AdmissionDispatcherCommand::Shutdown(receipt)) => {
                                receiver.close();
                                builds.abort_all();
                                let task_count = builds.len();
                                while let Some(result) = builds.join_next_with_id().await {
                                    match result {
                                        Ok((id, ())) => {
                                            terminalizers.remove(&id);
                                        }
                                        Err(error) => {
                                            if let Some(terminalize) = terminalizers.remove(&error.id()) {
                                                terminalize.await;
                                            }
                                        }
                                    }
                                }
                                terminalizers.terminalize_remaining().await;
                                let _ = receipt.send(task_count);
                                break;
                            }
                            None => {
                                builds.abort_all();
                                while let Some(result) = builds.join_next_with_id().await {
                                    match result {
                                        Ok((id, ())) => {
                                            terminalizers.remove(&id);
                                        }
                                        Err(error) => {
                                            if let Some(terminalize) = terminalizers.remove(&error.id()) {
                                                terminalize.await;
                                            }
                                        }
                                    }
                                }
                                terminalizers.terminalize_remaining().await;
                                break;
                            }
                        },
                        Some(result) = builds.join_next_with_id(), if !builds.is_empty() => {
                            match result {
                                Ok((id, ())) => {
                                    terminalizers.remove(&id);
                                }
                                Err(error) => {
                                    if let Some(terminalize) = terminalizers.remove(&error.id()) {
                                        terminalize.await;
                                    }
                                }
                            }
                        }
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

    pub(crate) fn spawn(
        &self,
        build: AdmissionBuildEnvelope,
    ) -> Result<(), AdmissionBuildEnvelope> {
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
