use docgen::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    Cli::init().await
}
