use crate::{
    code::downloader,
    generators::{
        rust_axum::{RustAxumGenerator, RustAxumGeneratorArgsBuilder},
        Generator,
    },
};
use anyhow::{bail, Context};
use args::{Args, Commands, Framework};
use clap::Parser;
use std::path::PathBuf;

pub mod args;
pub struct Cli;

impl Cli {
    pub fn init() -> anyhow::Result<()> {
        let args = Args::parse();

        if let Some(command) = args.command {
            match command {
                Commands::Generate {
                    url,
                    dir,
                    framework,
                } => {
                    let dir = match (dir, url) {
                        (Some(dir), None) => dir,
                        (None, Some(url)) => {
                            let download_dir = PathBuf::from("/temp/docgen/code");
                            downloader::download_from_url(&url, &download_dir)?;
                            download_dir
                        }
                        _ => bail!("either `--dir` or `--url` must be provided. Run docgen -h to check usage")
                    };

                    let generator = match framework {
                        Framework::RustAxum => {
                            let args = RustAxumGeneratorArgsBuilder::default()
                                .code_dir(dir)
                                .build()
                                .context("failed to build rust-axum args")?;
                            RustAxumGenerator::new(args)
                        }
                    };

                    let ir = generator.generate_ir()?;

                    println!("IR: {:?}", ir)
                }
            };
        }

        Ok(())
    }
}
