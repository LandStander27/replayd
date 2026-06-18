#![allow(clippy::needless_return)]

use replayd::prelude::*;

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
