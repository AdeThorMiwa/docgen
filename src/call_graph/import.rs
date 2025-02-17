use std::{
    collections::HashMap,
    fmt::Display,
    path::{Path, PathBuf},
};

use anyhow::anyhow;

#[derive(Debug)]
pub struct LocalImport {
    identifier: String,
    #[allow(unused)]
    pub full_path: String,
    pub module_file_path: PathBuf,
}

impl LocalImport {
    pub fn try_new(
        path_segments: &[String],
        base_dir: &Path,
        crate_name: &str,
    ) -> anyhow::Result<Self> {
        let module_file_path = Self::resolve_import_module_path(
            &path_segments[..path_segments.len() - 1],
            base_dir,
            crate_name,
        )
        .ok_or(anyhow!(
            "failed to resolve import module path or {}",
            path_segments.join("::")
        ))?;

        Ok(Self {
            identifier: path_segments.last().unwrap().to_owned(),
            full_path: path_segments.join("::"),
            module_file_path,
        })
    }

    pub fn resolve_import_module_path(
        segments: &[String],
        base_dir: &Path,
        crate_name: &str,
    ) -> Option<PathBuf> {
        let Some(first) = segments.first() else {
            return None;
        };

        let (mut module_dir, skip_segment) = match first.as_str() {
            first if first == crate_name || first == "crate" => {
                // src directory
                let dir = base_dir
                    .ancestors()
                    .find(|d| d.join("src").exists())
                    .map(|d| d.join("src"))?;
                (dir, 1)
            }
            "self" => (base_dir.to_path_buf(), 1),
            "super" => (base_dir.parent()?.to_path_buf(), 1),
            _ => return None,
        };

        for seg in &segments[skip_segment..segments.len() - 1] {
            module_dir = module_dir.join(seg);
        }

        let module = segments.last()?;
        let file_rs = module_dir.join(format!("{}.rs", module));
        let mod_rs = module_dir.join(module).join("mod.rs");

        if file_rs.exists() {
            Some(file_rs)
        } else if mod_rs.exists() {
            Some(mod_rs)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct ExternalImport {
    identifier: String,
    #[allow(unused)]
    pub full_path: String,
}

impl ExternalImport {
    pub fn new(path_segments: &[String]) -> Self {
        Self {
            identifier: path_segments[0].to_owned(),
            full_path: path_segments.join("::"),
        }
    }
}

#[derive(Debug)]
pub enum Import {
    Local(LocalImport),
    External(ExternalImport),
}

impl Import {
    pub fn get_identifier(&self) -> String {
        match self {
            Self::Local(l) => l.identifier.to_owned(),
            Self::External(e) => e.identifier.to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct ImportMap {
    imports: HashMap<String, Import>,
}

impl ImportMap {
    pub fn new() -> Self {
        Self {
            imports: HashMap::new(),
        }
    }

    pub fn insert(&mut self, value: Import) {
        self.imports.insert(value.get_identifier(), value);
    }

    pub fn get(&self, key: &str) -> Option<&Import> {
        self.imports.get(key)
    }
}

impl Display for ImportMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();

        for (key, value) in &self.imports {
            s.push_str(&format!("{}: {:?} \n", key, value));
        }

        write!(f, "{}", s)
    }
}
