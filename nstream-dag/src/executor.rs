use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use tracing::{error, info, info_span, Instrument};

use crate::dag::Pipeline;
use nstream_core::{JobStatus, MediaPayload, NStreamError};

/// Tracks the per-node execution status within a pipeline run.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NodeExecutionStatus {
    pub node_index: usize,
    pub node_name: String,
    pub status: JobStatus,
}

/// Wraps a `Pipeline` with execution tracking and tracing instrumentation.
pub struct PipelineExecutor {
    pipeline: Pipeline,
    #[allow(dead_code)]
    node_statuses: HashMap<NodeIndex, NodeExecutionStatus>,
}

impl PipelineExecutor {
    /// Create a new executor wrapping the given pipeline.
    pub fn new(pipeline: Pipeline) -> Self {
        Self {
            pipeline,
            node_statuses: HashMap::new(),
        }
    }

    /// Validate the underlying pipeline before execution.
    pub fn validate(&self) -> Result<(), NStreamError> {
        self.pipeline.validate()
    }

    /// Execute the pipeline, emitting tracing spans and tracking node statuses.
    pub async fn run(&self, payload: MediaPayload) -> Result<MediaPayload, NStreamError> {
        let job_id = payload.job_id;

        let span = info_span!(
            "pipeline_execution",
            job_id = %job_id,
            node_count = self.pipeline.node_count(),
            edge_count = self.pipeline.edge_count(),
        );

        async {
            info!(job_id = %job_id, "PipelineExecutor starting run");

            let result = self.pipeline.execute(payload).await;

            match &result {
                Ok(_) => info!(job_id = %job_id, "PipelineExecutor run succeeded"),
                Err(e) => error!(job_id = %job_id, error = %e, "PipelineExecutor run failed"),
            }

            result
        }
        .instrument(span)
        .await
    }

    /// Returns a reference to the underlying pipeline.
    pub fn pipeline(&self) -> &Pipeline {
        &self.pipeline
    }

    /// Returns a snapshot of all node execution statuses recorded so far.
    pub fn node_statuses(&self) -> &HashMap<NodeIndex, NodeExecutionStatus> {
        &self.node_statuses
    }
}
