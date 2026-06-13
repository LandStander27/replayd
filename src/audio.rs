use rodio::{DeviceSinkBuilder, MixerDeviceSink, Source};
use std::num::NonZero;

use crate::prelude::*;

#[derive(Clone)]
pub struct SharedSamples {
	samples: Arc<Vec<f32>>,
	pos: usize,
	channels: NonZero<u16>,
	sample_rate: NonZero<u32>,
}

impl SharedSamples {
	fn new(samples: Arc<Vec<f32>>, channels: NonZero<u16>, sample_rate: NonZero<u32>) -> Self {
		Self {
			samples,
			pos: 0,
			channels,
			sample_rate,
		}
	}
}

impl Iterator for SharedSamples {
	type Item = f32;

	fn next(&mut self) -> Option<f32> {
		let sample = self.samples.get(self.pos).copied();
		self.pos += 1;
		return sample;
	}
}

impl Source for SharedSamples {
	fn current_span_len(&self) -> Option<usize> {
		return None;
	}

	fn channels(&self) -> rodio::ChannelCount {
		return self.channels;
	}

	fn sample_rate(&self) -> rodio::SampleRate {
		return self.sample_rate;
	}

	fn total_duration(&self) -> Option<std::time::Duration> {
		return None;
	}
}

pub struct AudioPlayer {
	stream: MixerDeviceSink,
	samples: SharedSamples,
}

impl AudioPlayer {
	pub fn new(audio: &'static [u8]) -> Result<Self> {
		let cursor = std::io::Cursor::new(audio);
		let decoder = rodio::Decoder::new(cursor).context("could not decode audio")?;

		let channels = decoder.channels();
		let sample_rate = decoder.sample_rate();
		let samples = Arc::new(decoder.collect::<Vec<f32>>());

		let mut stream = DeviceSinkBuilder::open_default_sink().context("could not open sink")?;
		stream.log_on_drop(false);

		return Ok(Self {
			samples: SharedSamples::new(samples, channels, sample_rate),
			stream,
		});
	}

	pub fn play(&self) -> Result<()> {
		self.stream.mixer().add(self.samples.clone());
		return Ok(());
	}
}
