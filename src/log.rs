use tracing_subscriber::layer::SubscriberExt;

use crate::prelude::*;

pub fn init() -> Result<()> {
	let filter = tracing_subscriber::EnvFilter::builder()
		.parse(if !args::args().verbose {
			"info"
		} else {
			"trace"
		})
		.context("could not create filter")?;
	let subscriber = tracing_subscriber::fmt()
		.compact()
		.with_file(false)
		.with_line_number(false)
		.with_thread_ids(false)
		.with_target(true)
		.without_time()
		.with_env_filter(filter)
		.finish()
		.with(tracing_error::ErrorLayer::default());
	// .with(tracing_error::ErrorLayer::default());
	tracing::subscriber::set_global_default(subscriber).context("could not set global logger")?;

	return Ok(());
}
