use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, info};

use crate::tasks::{InferenceResult, InferenceTask};
use crate::worker::InferenceWorker;
use nstream_core::NStreamError;

/// A message passed through the worker pool channel.
struct PoolMessage {
    task: InferenceTask,
    reply: oneshot::Sender<Result<InferenceResult, NStreamError>>,
}

/// A pool of inference workers backed by a Tokio mpsc channel.
/// Spawns `worker_count` async workers at construction time.
pub struct WorkerPool {
    sender: mpsc::Sender<PoolMessage>,
    worker_count: usize,
}

impl WorkerPool {
    /// Create a new pool with `worker_count` workers, each configured with `model_path`.
    pub fn new(worker_count: usize, model_path: impl Into<std::path::PathBuf> + Clone) -> Self {
        // Channel capacity = 4× worker count to absorb short bursts.
        let (sender, receiver) = mpsc::channel::<PoolMessage>(worker_count * 4);
        let receiver = Arc::new(Mutex::new(receiver));

        for worker_id in 0..worker_count {
            let rx = Arc::clone(&receiver);
            let worker = InferenceWorker::new(worker_id as u32, model_path.clone());
            tokio::spawn(async move {
                info!(worker_id = worker_id, "Inference worker started");
                loop {
                    let msg = {
                        let mut guard = rx.lock().await;
                        guard.recv().await
                    };
                    match msg {
                        Some(PoolMessage { task, reply }) => {
                            let task_type = task.task_type();
                            debug!(
                                worker_id = worker_id,
                                task = task_type,
                                "Worker picked up task"
                            );
                            let result = worker.run_task(task).await;
                            if reply.send(result).is_err() {
                                error!(
                                    worker_id = worker_id,
                                    task = task_type,
                                    "Reply channel closed before result could be sent"
                                );
                            }
                        }
                        None => {
                            info!(
                                worker_id = worker_id,
                                "Worker pool channel closed — shutting down worker"
                            );
                            break;
                        }
                    }
                }
            });
        }

        info!(worker_count = worker_count, "WorkerPool initialized");
        Self {
            sender,
            worker_count,
        }
    }

    /// Submit an inference task to the pool.
    /// Returns the result asynchronously via a one-shot channel.
    pub async fn submit(&self, task: InferenceTask) -> Result<InferenceResult, NStreamError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let task_type = task.task_type();

        self.sender
            .send(PoolMessage {
                task,
                reply: reply_tx,
            })
            .await
            .map_err(|e| NStreamError::ChannelSend(format!("WorkerPool send failed: {}", e)))?;

        debug!(task = task_type, "Task submitted to worker pool");

        reply_rx.await.map_err(|e| {
            NStreamError::ChannelSend(format!("WorkerPool reply channel dropped: {}", e))
        })?
    }

    /// Returns the number of workers in the pool.
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
}
