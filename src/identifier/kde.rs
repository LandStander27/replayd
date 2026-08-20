use zbus::{Connection, Proxy, zvariant::OwnedValue};

use crate::prelude::*;

pub struct Kde;

#[async_trait]
impl WindowManager for Kde {
	async fn get_focused_window(&self) -> Result<Window> {
		let id = run_proc(["getactivewindow"])
			.await
			.context("could not get active window id")?;

		return info(&id).await;
	}
}

async fn info(id: &str) -> Result<Window> {
	let connection = Connection::session().await?;
	let proxy = Proxy::new(&connection, "org.kde.KWin", "/KWin", "org.kde.KWin").await?;
	let info: HashMap<String, OwnedValue> = proxy.call("getWindowInfo", &(id,)).await?;

	let title = info
		.get("caption")
		.context("missing title")?
		.clone()
		.try_into()
		.context("title is not a String")?;
	let class = info
		.get("resourceClass")
		.context("missing class")?
		.clone()
		.try_into()
		.context("class is not a String")?;
	let fullscreen = info
		.get("fullscreen")
		.context("missing fullscreen")?
		.to_string()
		.parse()
		.context("invalid fulscreen bool")?;
	let pid = info
		.get("pid")
		.context("missing pid")?
		.to_string()
		.parse()
		.context("invalid pid")?;

	let (executable, cmdline) = super::get_cmdline(pid).await?;

	return Ok(Window {
		title,
		executable,
		cmdline,
		class,
		fullscreen,
	});
}

async fn run_proc<I: IntoIterator<Item = S>, S: AsRef<std::ffi::OsStr>>(args: I) -> Result<String> {
	let proc = Command::new("/usr/bin/kdotool")
		.args(args)
		.stdout(Stdio::piped())
		.stdin(Stdio::null())
		.stderr(Stdio::piped())
		.spawn()
		.context("could not exec /usr/bin/kdotool")?;

	let output = proc
		.wait_with_output()
		.await
		.context("could not wait for kdotool")?;

	let stderr = String::from_utf8_lossy(&output.stderr).to_string();
	let stdout = String::from_utf8_lossy(&output.stdout).to_string();

	if !output.status.success() {
		let code = output
			.status
			.code()
			.map(|x| x.to_string())
			.unwrap_or(String::from("unknown"));

		return Err(eyre!("{stderr}\n\nkdotool returned {code} exit-code"));
	}

	if stderr.chars().any(|c| !c.is_whitespace()) {
		warn!("kdotool stderr: {stderr}");
	}

	return Ok(stdout);
}
