use crate::domain::ir::IR;

pub mod rust_axum;

pub trait Generator {
    /// Generates an intermediate representation (`IR`) of our eventual documentation spec
    fn generate_ir(&self) -> anyhow::Result<IR>;
}
