use clap::Parser;
use tokio::sync::OnceCell;

use crate::prelude::*;

static ARGS: OnceCell<Args> = OnceCell::const_new();

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "replayd", disable_help_flag = true, disable_version_flag = true, version = version::version)]
#[command(about = "clip your favorite moments!\nrepository: https://codeberg.org/Land/replayd", long_about = None)]
pub struct Args {
	#[arg(long, help = "display help", action = clap::builder::ArgAction::Help)]
	pub help: (),

	#[arg(long, help = "print version")]
	pub version: bool,

	#[arg(short, long, help = "increase verbosity")]
	pub verbose: bool,

	#[arg(long, help = "open the window minimized")]
	pub open_minimized: bool,
}

pub async fn parse() {
	ARGS.get_or_init(async || {
		let args = Args::parse();

		if args.version {
			eprintln!("replayd {}", version::version);
			eprintln!("{}", Recorder::get_version().await);
			std::process::exit(0);
		}

		args
	})
	.await;
}

pub fn args() -> &'static Args {
	return ARGS.get().unwrap();
}
