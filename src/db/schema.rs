use crate::prelude::*;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumProperty, FromRepr};

macro_rules! impl_serde {
	($ty:ty) => {
		impl $ty {
			pub fn encode(&self) -> Result<Vec<u8>> {
				return rmp_serde::to_vec(self).context("could not encode data");
			}

			pub fn decode(value: &[u8]) -> Result<$ty> {
				return rmp_serde::from_slice(value).context("could not decode json");
			}
		}

		impl TryFrom<&[u8]> for $ty {
			type Error = color_eyre::eyre::Report;

			fn try_from(value: &[u8]) -> Result<$ty> {
				return Self::decode(value);
			}
		}

		impl TryFrom<$ty> for Vec<u8> {
			type Error = color_eyre::eyre::Report;

			fn try_from(value: $ty) -> Result<Vec<u8>> {
				return value.encode();
			}
		}
	};
}

pub type ObjectId = u64;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Clip {
	pub id: ObjectId,
	pub game: Option<ObjectId>,
	pub title: String,
	pub path: PathBuf,
	pub created: u64,
	pub duration_secs: Option<u64>,
	pub codec: Codec,
	pub container: Container,
	pub resolution: Resolution,
	pub quality: Quality,
	pub fps: FrameRate,

	#[serde(default)]
	pub favorited: bool,
}

impl Clip {
	pub fn uri(&self, library: &Path) -> Result<String> {
		return url::Url::from_file_path(self.absolute_path(library))
			.ok()
			.context("could not convert to file uri path")
			.map(|x| x.into());
	}

	pub fn absolute_path(&self, library: &Path) -> PathBuf {
		return library.join(&self.path);
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
pub struct Game {
	pub id: ObjectId,
	pub game_id: ObjectId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, EnumProperty, EnumIter)]
pub enum Resolution {
	#[strum(props(cmd = "2560x1440", display = "1440p"))]
	P1440,

	#[strum(props(cmd = "1920x1080", display = "1080p"))]
	#[default]
	P1080,

	#[strum(props(cmd = "1280x720", display = "720p"))]
	P720,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, EnumProperty, EnumIter)]
pub enum Codec {
	#[strum(props(cmd = "h264", display = "H.264"))]
	#[default]
	H264,

	#[strum(props(cmd = "av1", display = "AV1"))]
	AV1,

	#[strum(props(cmd = "hevc", display = "HEVC"))]
	HEVC,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, EnumProperty, EnumIter)]
pub enum Container {
	#[strum(props(cmd = "mp4", display = "MP4"))]
	#[default]
	MP4,

	#[strum(props(cmd = "mkv", display = "MKV"))]
	MKV,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, EnumProperty, EnumIter)]
pub enum Quality {
	#[strum(props(cmd = "medium", display = "Medium"))]
	Medium,

	#[strum(props(cmd = "high", display = "High"))]
	#[default]
	High,

	#[strum(props(cmd = "very_high", display = "Very High"))]
	VeryHigh,

	#[strum(props(cmd = "ultra", display = "Ultra"))]
	Ultra,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, EnumProperty, EnumIter)]
pub enum FrameRate {
	#[strum(props(cmd = "24", display = "24"))]
	Fps24,

	#[strum(props(cmd = "30", display = "30"))]
	Fps30,

	#[strum(props(cmd = "60", display = "60"))]
	#[default]
	Fps60,

	#[strum(props(cmd = "120", display = "120"))]
	Fps120,

	#[strum(props(cmd = "144", display = "144"))]
	Fps144,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAction {
	pub id: ObjectId,
	pub name: String,
	pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
	pub buffer_length: u64,
	pub codec: Codec,
	pub resolution: Resolution,
	pub container: Container,
	pub display: Option<String>,
	pub frame_rate: FrameRate,
	pub quality: Quality,
	pub output_dir: PathBuf,
	pub show_end_title_buttons: bool,
	pub notifications: bool,
	pub sound_feedback: bool,

	#[serde(default)]
	pub custom_actions: Vec<CustomAction>,
}

impl Default for Settings {
	fn default() -> Self {
		return Self {
			buffer_length: 60,
			codec: Codec::default(),
			resolution: Resolution::default(),
			container: Container::default(),
			quality: Quality::default(),
			frame_rate: FrameRate::default(),
			display: None,
			output_dir: if cfg!(debug_assertions) {
				std::env::current_dir().unwrap().join("clips")
			} else {
				dirs::video_dir()
					.unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Videos"))
					.join("Clips")
			},
			show_end_title_buttons: true,
			notifications: false,
			sound_feedback: true,
			custom_actions: Vec::new(),
		};
	}
}

impl_serde!(Clip);
impl_serde!(Game);
impl_serde!(Settings);
