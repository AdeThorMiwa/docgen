use anyhow::Context;
use cargo_toml::Manifest as CargoManifest;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Manifest {
    manifest: CargoManifest,
}

impl Manifest {
    pub fn try_new(root_dir: &PathBuf) -> anyhow::Result<Self> {
        let manifest = CargoManifest::from_path(root_dir.join("Cargo.toml"))
            .context(format!("failed to read Cargo.toml at {:?}", root_dir))?;

        Ok(Self { manifest })
    }

    pub fn package_name(&self) -> Option<String> {
        self.manifest.package.clone().map(|p| p.name.to_owned())
    }
}
