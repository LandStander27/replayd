#![allow(clippy::needless_return)]

pub mod args;
pub mod audio;
pub mod db;
pub mod identifier;
pub mod log;
pub mod portals;
pub mod recorder;
pub mod search;
pub mod socket;
pub mod thumbnail;
pub mod window;

pub mod prelude;
use prelude::*;

fn main() -> Result<()> {
	color_eyre::install().context("could not install handler")?;

	tokio::runtime::Builder::new_current_thread()
		.enable_io()
		.build_local(tokio::runtime::LocalOptions::default())
		.context("could not create tokio runtime")?
		.block_on(args::parse());

	log::init().context("could not init logger")?;
	window::root::run().context("app failed")?;

	return Ok(());
}
