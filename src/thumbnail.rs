use gstreamer::prelude::*;

use crate::prelude::*;

pub fn cache_path(clip_path: &Path) -> Result<PathBuf> {
	let hash = format!("{:x}", md5::compute(clip_path.to_string_lossy().as_bytes()));
	return Ok(dirs::cache_dir()
		.context("could not get XDG_CACHE_DIR")?
		.join("replayd")
		.join("thumbs")
		.join(format!("{hash}.jpg")));
}

pub fn extract(clip_path: &Path) -> Result<PathBuf> {
	let dest = cache_path(clip_path)?;
	if dest.exists() {
		return Ok(dest);
	}

	std::fs::create_dir_all(dest.parent().unwrap())?;

	gstreamer::init()?;

	let pipeline = format!(
		"uridecodebin uri=\"file://{}\" ! videoconvert ! videoscale ! video/x-raw,width=320,height=180 ! jpegenc ! filesink location=\"{}\"",
		clip_path.display(),
		dest.display()
	);

	let pipeline = gstreamer::parse::launch(&pipeline)?
		.downcast::<gstreamer::Pipeline>()
		.unwrap();

	pipeline.set_state(gstreamer::State::Paused)?;
	pipeline
		.state(gstreamer::ClockTime::from_seconds(5))
		.0
		.context("could not set video state to paused")?;

	let duration = pipeline
		.query_duration::<gstreamer::ClockTime>()
		.unwrap_or(gstreamer::ClockTime::from_seconds(10));

	let seek_pos = duration / 10;
	pipeline.seek_simple(gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT, seek_pos)?;

	pipeline.set_state(gstreamer::State::Playing)?;

	let bus = pipeline.bus().unwrap();
	for msg in bus.iter_timed(gstreamer::ClockTime::from_seconds(10)) {
		use gstreamer::MessageView;
		match msg.view() {
			MessageView::Eos(..) => break,
			MessageView::Error(e) => return Err(eyre!("{e}")),
			_ => {}
		}
	}

	pipeline.set_state(gstreamer::State::Null)?;
	return Ok(dest);
}
