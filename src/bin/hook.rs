#![allow(clippy::needless_return)]

pub use color_eyre::{
	Result,
	eyre::{Context, ContextCompat, eyre},
};
use std::io::Write;
use std::os::unix::net::UnixStream;

fn main() -> Result<()> {
	color_eyre::install().context("could not install handler")?;

	let mut args = std::env::args();
	if args.len() != 3 {
		return Err(eyre!("malformed arguments"));
	}

	let clip = args.nth(1).unwrap();

	let socket_path = dirs::runtime_dir()
		.context("could not find XDG_RUNTIME_DIR")?
		.join("replayd")
		.join("socket.sock");

	let mut stream = UnixStream::connect(&socket_path).context("could not open stream")?;
	stream
		.write_all(clip.as_bytes())
		.context("could not write to stream")?;

	println!("success");
	return Ok(());
}
