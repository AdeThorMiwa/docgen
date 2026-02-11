use std::path::PathBuf;

use async_trait::async_trait;

use crate::domain::ir::{Route, IR};

pub mod rust_axum;

#[async_trait]
pub trait Generator {
    /// Generates an intermediate representation (`IR`) of our eventual documentation spec
    async fn generate_ir(&self) -> anyhow::Result<IR>;
}

pub struct GeneratorBaseInfo {}
pub type GeneratorRoute = Route;

#[async_trait]
pub trait Gen {
    async fn get_base_info(&mut self) -> anyhow::Result<GeneratorBaseInfo>;
    async fn find_route_files(&mut self) -> anyhow::Result<PathBuf>;
    async fn get_routes_from_file(
        &mut self,
        file_path: &PathBuf,
    ) -> anyhow::Result<Vec<GeneratorRoute>>;
}
