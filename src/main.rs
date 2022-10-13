use anyhow::Result;
use clap::{ArgEnum, Parser};
use pgsubset::config::Config;
use sqlx::postgres::PgPoolOptions;

use std::path::PathBuf;

#[derive(Parser)]
#[clap(version, about)]
struct Args {
    #[clap(short, long)]
    config: PathBuf,
    #[clap(arg_enum, short, long)]
    mode: Mode,
}

#[derive(ArgEnum, Clone)]
enum Mode {
    Export,
    Import,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    let cfg_content = tokio::fs::read_to_string(args.config).await?;
    let cfg: Config = toml::from_str(&cfg_content)?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.database_url)
        .await?;

    match args.mode {
        Mode::Export => pgsubset::run::export(&pool, cfg).await?,
        Mode::Import => pgsubset::run::import(&pool, cfg).await?,
    }
    Ok(())
}
