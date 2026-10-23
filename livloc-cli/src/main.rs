#![allow(clippy::single_match, clippy::collapsible_match)]

use crate::{app::run_app, livloc_app::LivlocApp};

mod app;

mod flame_graph;
mod livloc_app;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let mut app = LivlocApp::with(Default::default(), Default::default()).await?;
    run_app(&mut app).await?;

    Ok(())
}
