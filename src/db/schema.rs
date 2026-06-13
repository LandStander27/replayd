use crate::prelude::*;
use serde::{Deserialize, Serialize};
use strum::{Display, FromRepr};

macro_rules! impl_serde {
	($ty:ty) => {
		impl $ty {
			pub fn encode(&self) -> Result<Vec<u8>> {
				return serde_json::to_vec(self).context("could not encode data");
			}

			pub fn decode(value: &[u8]) -> Result<$ty> {
				return serde_json::from_slice(value).context("could not decode json");
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
	pub path_display: String,
	pub path_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
	pub id: ObjectId,
	pub window_class: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, Display)]
pub enum Resolution {
	#[strum(to_string = "2560x1440")]
	P1440,

	#[strum(to_string = "1920x1080")]
	#[default]
	P1080,

	#[strum(to_string = "1080x720")]
	P720,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, Display)]
pub enum Codec {
	#[strum(to_string = "h264")]
	#[default]
	H264,

	#[strum(to_string = "av1")]
	AV1,

	#[strum(to_string = "hevc")]
	HEVC,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, Display)]
pub enum Container {
	#[strum(to_string = "mp4")]
	#[default]
	MP4,

	#[strum(to_string = "mkv")]
	MKV,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, Display)]
pub enum Quality {
	#[strum(to_string = "medium")]
	Medium,

	#[strum(to_string = "high")]
	#[default]
	High,

	#[strum(to_string = "very_high")]
	VeryHigh,

	#[strum(to_string = "ultra")]
	Ultra,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, FromRepr, Display)]
pub enum FrameRate {
	#[strum(to_string = "24")]
	Fps24,

	#[strum(to_string = "30")]
	Fps30,

	#[strum(to_string = "60")]
	#[default]
	Fps60,

	#[strum(to_string = "120")]
	Fps120,

	#[strum(to_string = "144")]
	Fps144,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
	pub buffer_length: i32,
	pub codec: Codec,
	pub resolution: Resolution,
	pub container: Container,
	pub display: Option<String>,
	pub frame_rate: FrameRate,
	pub quality: Quality,
	pub output_dir: String,
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
			output_dir: format!("{}/Videos/Clips", std::env::var("HOME").unwrap()),
		};
	}
}

impl_serde!(Clip);
impl_serde!(Game);
impl_serde!(Settings);
