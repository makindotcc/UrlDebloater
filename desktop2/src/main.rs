mod app;
mod clipboard;
mod config;
mod tray;

use log::info;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting application");

    app::run().await
}
