pub mod full_pipeline;
pub mod manager;
pub mod pool;
pub mod remote;
pub mod tasks;
pub mod worker;

pub use full_pipeline::{FullPipelineClient, FullPipelineHealth, PipelineCapabilities};
pub use manager::{InferenceManager, InferenceMode};
pub use pool::WorkerPool;
pub use remote::{HealthResponse, RemoteWhisperAsr};
pub use tasks::{InferenceResult, InferenceTask};
pub use worker::{InferenceWorker, TranscriptSegment, WhisperAsr};
