mod app;
mod config;
mod error;
#[cfg(target_os = "linux")]
mod lock_screen;
mod util;

use crate::app::App;
use crate::config::Config;
use crate::error::Error;
use clap::Parser;
use env_logger::Env;
use log::debug;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Set a custom config file
    #[clap(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
    }
}

fn run() -> Result<(), Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let config_path = args.config.unwrap_or({
        dirs::config_dir()
            .ok_or_else(|| Error::Custom("No config directory found on your system.".to_string()))?
            .join("blink")
            .join("blink.yaml")
    });
    debug!("Config path: '{}'", config_path.display());

    let config = Config::load_or_create(config_path)?;
    App::new(config).run();

    Ok(())
}
