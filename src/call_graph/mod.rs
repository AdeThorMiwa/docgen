use async_trait::async_trait;
use petgraph::graph::DiGraph;
use proc_macro2::LineColumn;
use std::path::PathBuf;

pub mod gpt_graph_builder;
pub mod graph;
pub mod import;
pub mod manifest;

pub struct LocationInfo {
    pub start: LineColumn,
    pub end: LineColumn,
}

pub struct NodeDefinition {
    pub file: PathBuf,
    pub location: LocationInfo,
}

pub struct GraphNode {
    pub parent_struct: Option<String>,
    pub fn_identifier: String,
    pub definition: NodeDefinition,
}

pub struct GraphEdge {
    pub call_site: LocationInfo,
}

#[async_trait]
pub trait CallGraphBuilder {
    async fn build(&mut self) -> anyhow::Result<DiGraph<GraphNode, GraphEdge>>;
}
