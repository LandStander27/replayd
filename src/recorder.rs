use crate::prelude::*;

#[derive(Default)]
pub struct Recorder {
	process: Option<Child>,
}

impl Recorder {
	pub fn new() -> Self {
		return Self::default();
	}

	pub async fn get_version() -> String {
		let proc = match Command::new("gpu-screen-recorder")
			.arg("--version")
			.kill_on_drop(true)
			.stderr(Stdio::null())
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.spawn()
		{
			Ok(o) => o,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				return "gpu-screen-recorder not found!".to_string();
			}
			Err(e) => panic!("{e}"),
		};

		let output = proc.wait_with_output().await.unwrap(); // if this fails, we want to panic
		if !output.status.success() {
			return "gpu-screen-recorder 0.0.0 (invalid output)".to_string();
		}

		let version = String::from_utf8_lossy(&output.stdout);
		return format!("gpu-screen-recorder {}", version.trim());
	}

	pub fn is_active(&self) -> bool {
		return self.process.is_some();
	}

	pub fn start(&mut self, settings: &Settings) -> Result<()> {
		if self.process.is_some() {
			return Err(eyre!("gpu-screen-recorder already running"));
		}

		let hook_path = std::env::current_exe()
			.context("could not get current executable")?
			.parent()
			.context("no parent directory found")?
			.join("replayd-hook");

		let process = Command::new("gpu-screen-recorder")
			.args([
				"-s",
				&settings.resolution.to_string(),
				"-w",
				settings.display.as_ref().context("no display specified")?,
				"-v",
				"no",
				"-f",
				&settings.frame_rate.to_string(),
				"-r",
				&settings.buffer_length.to_string(),
				"-k",
				&settings.codec.to_string(),
				"-c",
				&settings.container.to_string(),
				"-a",
				"default_output|default_input",
				"-q",
				&settings.quality.to_string(),
				"-bm",
				"qp",
				"-encoder",
				"gpu",
				"-sc",
				&hook_path.display().to_string(),
				"-o",
				&settings.output_dir,
			])
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::piped()) // TODO: set bridge to tracing debug!
			.kill_on_drop(true)
			.spawn()
			.context("could not spawn gpu-screen-recorder")?;

		self.process = Some(process);
		return Ok(());
	}

	pub fn stop(&mut self) -> Result<()> {
		let proc = self
			.process
			.take()
			.context("gpu-screen-recorder not running")?;

		let pid = proc.id().context("gpu-screen-recorder not running")?;
		nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::SIGINT).context("could not send SIGKILL to gpu-screen-recorder")?;

		return Ok(());
	}

	pub fn clip(&self) -> Result<()> {
		if !self.is_active() {
			return Ok(());
		}

		let proc = self
			.process
			.as_ref()
			.context("gpu-screen-recorder not running")?;

		let pid = proc.id().context("gpu-screen-recorder not running")?;
		nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::SIGUSR1).context("could not send SIGUSR1 to gpu-screen-recorder")?;

		return Ok(());
	}

	pub fn toggle(&mut self, settings: &Settings) -> Result<()> {
		if self.process.is_none() {
			return self.start(settings);
		}

		return self.stop();
	}
}

pub fn get_displays() -> Result<Vec<String>> {
	let displays = display_info::DisplayInfo::all().context("could not get displays")?;
	let mut ret: Vec<String> = displays.into_iter().map(|x| x.name).collect();
	ret.sort();
	return Ok(ret);

	// return Ok(displays
	// 	.into_iter()
	// 	.fold(BTreeMap::new(), |mut map, display| {
	// 		map.insert(display.id, display.name);
	// 		map
	// 	}));
}

// pub fn get_display(id: u32) -> Result<String> {
// 	let displays = display_info::DisplayInfo::all().context("could not get displays")?;
// 	return Ok(displays
// 		.into_iter()
// 		.find(|x| x.id == id)
// 		.context("display not found")?
// 		.name);
// }
