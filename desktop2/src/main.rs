mod app;
mod clipboard;
mod config;
mod tray;

use log::info;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting application");

    app::run()
}
