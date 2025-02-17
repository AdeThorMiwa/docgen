use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Clone, ValueEnum)]
pub enum Framework {
    RustAxum,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Generate document for a given codebase")]
    Generate {
        #[arg(short, long)]
        url: Option<Url>,
        #[arg(short, long)]
        dir: Option<PathBuf>,
        #[arg(short, long, value_enum)]
        framework: Framework,
    },
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = "Docgen CLI")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long)]
    pub verbose: bool,
}
