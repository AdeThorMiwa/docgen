use anyhow::Ok;
use async_trait::async_trait;
use petgraph::graph::DiGraph;

use super::CallGraphBuilder;

pub struct GPTGraphBuilder {
    graph: DiGraph<super::GraphNode, super::GraphEdge>,
}

impl GPTGraphBuilder {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
        }
    }
}

#[async_trait]
impl CallGraphBuilder for GPTGraphBuilder {
    async fn build(
        &mut self,
    ) -> anyhow::Result<petgraph::prelude::DiGraph<super::GraphNode, super::GraphEdge>> {
        Ok(DiGraph::new())
    }
}
