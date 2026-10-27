#![allow(clippy::single_match, clippy::collapsible_match)]

use clap::Parser;
use reqwest::Url;

use crate::{
    app::run_app,
    livloc_app::{AppOptions, LivlocApp},
};

mod app;

mod flame_graph;
mod livloc_app;

#[derive(Parser)]
struct Args {
    #[clap(long)]
    tree_url: Option<Url>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();

    color_eyre::install()?;

    let mut opts = AppOptions::default();
    if let Some(x) = args.tree_url {
        opts.tree_url = x;
    }

    let mut app = LivlocApp::with(Default::default(), opts).await?;
    run_app(&mut app).await?;

    Ok(())
}
