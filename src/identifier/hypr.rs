use crate::prelude::*;

pub struct Hyprland;

#[async_trait]
impl WindowManager for Hyprland {
	async fn get_focused_window(&self) -> Result<Window> {
		let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").context("could not get HYPRLAND_INSTANCE_SIGNATURE")?;
		let socket_path = dirs::runtime_dir()
			.context("could not get XDG_RUNTIME_DIR")?
			.join("hypr")
			.join(instance)
			.join(".socket.sock");

		let stream = UnixStream::connect(&socket_path)
			.await
			.context("could not open hypr stream")?;

		let (mut read, mut write) = stream.into_split();

		write
			.write_all(b"j/activewindow")
			.await
			.context("could not write to hypr socket")?;
		write
			.shutdown()
			.await
			.context("could not shutdown writing")?;

		let mut json = String::new();
		read.read_to_string(&mut json)
			.await
			.context("could not read from hypr socket")?;

		let window: serde_json::Value = serde_json::from_str(&json).context("could not parse json from hypr")?;
		let pid = window["pid"].as_u64().context("no pid in window json")?;
		let cmdline = PathBuf::from("/proc").join(format!("{pid}")).join("comm");
		let mut cmdline: Vec<String> = std::fs::read_to_string(&cmdline)
			.with_context(|| format!("could not read {cmdline:?}"))
			.map(|x| x.split('\0').map(|x| x.to_string()).collect())
			.show_error()
			.unwrap_or_default();
		let executable = if cmdline.is_empty() {
			None
		} else {
			Some(cmdline.remove(0))
		};

		return Ok(Window {
			class: window["class"]
				.as_str()
				.context("no class in window json")?
				.to_string(),
			title: window["title"]
				.as_str()
				.context("no title in window json")?
				.to_string(),
			fullscreen: window["fullscreen"]
				.as_i64()
				.context("no fullscreen in window json")?
				>= 2,
			executable,
			cmdline,
		});
	}
}
