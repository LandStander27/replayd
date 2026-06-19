#![allow(clippy::needless_return)]
#![cfg(feature = "socket_commands")]

use clap::{Parser, Subcommand};
pub use color_eyre::{
	Result,
	eyre::{Context, ContextCompat, eyre},
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[derive(Parser, Debug, Clone)]
#[command(name = "replaydctl", disable_help_flag = true, disable_version_flag = true, version = version::version)]
#[command(about = "controller for a replayd instance\nrepository: https://codeberg.org/Land/replayd", long_about = None)]
pub struct Args {
	#[arg(long, help = "display help", action = clap::builder::ArgAction::Help)]
	pub help: (),

	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
	/// view the database for the app
	Db,

	/// view the identifiable games
	Games,

	/// tell replayd to save a clip
	Clip,

	/// tell replayd to toggle clipping
	Toggle,

	/// get the currently identified game
	Identify,
}

#[tokio::main]
async fn main() -> Result<()> {
	color_eyre::install().context("could not install handler")?;

	let args = Args::parse();

	let socket_path = dirs::runtime_dir()
		.context("could not find XDG_RUNTIME_DIR")?
		.join("replayd")
		.join("socket.sock");

	match args.command {
		Commands::Db => {
			if !socket_path.exists() {
				eprintln!("{socket_path:?} does not exist\nare you sure Replayd is running?");
				std::process::exit(1);
			}
			let mut stream = UnixStream::connect(&socket_path).context("could not open stream")?;
			stream
				.write_all(b"get/db")
				.context("could not write to socket")?;
			stream
				.shutdown(std::net::Shutdown::Write)
				.context("could not shutdown writing")?;

			let mut response = String::new();
			stream
				.read_to_string(&mut response)
				.context("could not read from socket")?;

			print!("{response}");
		}
		Commands::Games => {
			if !socket_path.exists() {
				eprintln!("{socket_path:?} does not exist\nare you sure Replayd is running?");
				std::process::exit(1);
			}
			let mut stream = UnixStream::connect(&socket_path).context("could not open stream")?;
			stream
				.write_all(b"get/games")
				.context("could not write to socket")?;
			stream
				.shutdown(std::net::Shutdown::Write)
				.context("could not shutdown writing")?;

			let mut response = String::new();
			stream
				.read_to_string(&mut response)
				.context("could not read from socket")?;

			print!("{response}");
		}
		Commands::Identify => {
			let window_manager = replayd::identifier::get_window_manager().context("could not find window manager")?;
			let window = window_manager
				.get_focused_window()
				.await
				.context("could not get current window")?;

			println!("Window: {window:#?}\n");
			replayd::identifier::get_games().await?;
			if let Some(game) = replayd::identifier::identify_game(&window) {
				let game = replayd::identifier::get_game(game).context("invalid game")?;
				println!("{}", game.name);
			} else {
				println!("Game is unknown");
			}
		}
		Commands::Clip => {
			if !socket_path.exists() {
				eprintln!("{socket_path:?} does not exist\nare you sure Replayd is running?");
				std::process::exit(1);
			}
			let mut stream = UnixStream::connect(&socket_path).context("could not open stream")?;
			stream
				.write_all(b"signal/clip")
				.context("could not write to socket")?;
		}
		Commands::Toggle => {
			if !socket_path.exists() {
				eprintln!("{socket_path:?} does not exist\nare you sure Replayd is running?");
				std::process::exit(1);
			}
			let mut stream = UnixStream::connect(&socket_path).context("could not open stream")?;
			stream
				.write_all(b"signal/toggle")
				.context("could not write to socket")?;
		}
	}

	return Ok(());
}
