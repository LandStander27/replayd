#![allow(clippy::needless_return)]

pub mod args;
pub mod audio;
pub mod db;
pub mod identifier;
pub mod listener;
pub mod log;
pub mod portals;
pub mod recorder;
pub mod thumbnail;
pub mod window;

pub mod prelude;
use prelude::*;

fn main() -> Result<()> {
	color_eyre::install().context("could not install handler")?;

	{
		let runtime = tokio::runtime::Builder::new_current_thread()
			.enable_io()
			.build_local(tokio::runtime::LocalOptions::default())
			.context("could not create tokio runtime")?;
		runtime.block_on(async {
			args::args().await; // ensures args are valid

			log::init().await.context("could not init logger")
		})?;
	}

	window::root::run().context("app failed")?;

	return Ok(());
}
