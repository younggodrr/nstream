use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use nstream_core::{MediaPayload, NStreamError, Node};

/// A directed acyclic pipeline of processing nodes.
/// Nodes are executed in topological order.
pub struct Pipeline {
    graph: DiGraph<Box<dyn Node>, ()>,
    node_names: HashMap<NodeIndex, String>,
}

impl Pipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_names: HashMap::new(),
        }
    }

    /// Add a node to the pipeline, returning its index.
    pub fn add_node(&mut self, node: Box<dyn Node>) -> NodeIndex {
        let name = node.name().to_string();
        let idx = self.graph.add_node(node);
        self.node_names.insert(idx, name);
        debug!(node_index = idx.index(), node_name = %self.node_names[&idx], "Added node to pipeline");
        idx
    }

    /// Add a directed edge from `from` to `to`, meaning `to` depends on `from`.
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex) {
        self.graph.add_edge(from, to, ());
        debug!(
            from = from.index(),
            to = to.index(),
            "Added edge to pipeline"
        );
    }

    /// Validate the pipeline by detecting cycles.
    /// Returns an error if a cycle is found.
    pub fn validate(&self) -> Result<(), NStreamError> {
        match toposort(&self.graph, None) {
            Ok(_) => {
                info!("Pipeline validation passed — no cycles detected");
                Ok(())
            }
            Err(cycle) => {
                let node_idx = cycle.node_id();
                let name = self
                    .node_names
                    .get(&node_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("<unknown>");
                warn!(node = name, "Cycle detected in pipeline");
                Err(NStreamError::Pipeline(format!(
                    "Cycle detected at node '{}' (index {})",
                    name,
                    node_idx.index()
                )))
            }
        }
    }

    /// Execute the pipeline on the given payload.
    /// Nodes run sequentially in topological order, each receiving the output of the previous.
    pub async fn execute(
        &self,
        initial_payload: MediaPayload,
    ) -> Result<MediaPayload, NStreamError> {
        let order = toposort(&self.graph, None).map_err(|e| {
            NStreamError::Pipeline(format!(
                "Topological sort failed at node index {}",
                e.node_id().index()
            ))
        })?;

        info!(
            job_id = %initial_payload.job_id,
            node_count = order.len(),
            "Starting pipeline execution"
        );

        let mut payload = initial_payload;

        for node_idx in order {
            let node = &self.graph[node_idx];
            let node_name = self
                .node_names
                .get(&node_idx)
                .map(|s| s.as_str())
                .unwrap_or("<unknown>");

            info!(node = node_name, job_id = %payload.job_id, "Executing node");

            payload = node.process(payload).await.map_err(|e| {
                NStreamError::Pipeline(format!("Node '{}' failed: {}", node_name, e))
            })?;

            debug!(node = node_name, "Node execution complete");
        }

        info!(job_id = %payload.job_id, "Pipeline execution complete");
        Ok(payload)
    }

    /// Returns the number of nodes in the pipeline.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of edges in the pipeline.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
