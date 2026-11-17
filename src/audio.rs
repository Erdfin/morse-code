//! Real-time sine-wave synthesizer on top of cpal.
//!
//! One oscillator is shared by the sidetone (straight key) and sequence
//! playback (receive mode). The tone is gated through a raised-cosine
//! envelope with a short attack/release ramp so starting and stopping the
//! tone doesn't click.

use std::f32::consts::{PI, TAU};
use std::sync::{Arc, Mutex, MutexGuard};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use morse_trainer::morse::Element;

/// Attack/release ramp length.
const RAMP_MS: f32 = 5.0;
/// Silence before a played sequence, so the first dit isn't swallowed while
/// the output device wakes up.
const LEAD_IN_MS: f32 = 150.0;

#[derive(Clone, Copy, Debug)]
struct Segment {
	on: bool,
	samples: u64,
}

struct Synth {
	sample_rate: f32,
	freq: f32,
	volume: f32,
	key_down: bool,

	seq: Vec<Segment>,
	idx: usize,
	remaining: u64,
	playing: bool,
	paused: bool,

	phase: f32,
	/// Linear envelope position 0..1; shaped with a raised cosine on output.
	env: f32,
	ramp_step: f32,
}

impl Synth {
	fn new(sample_rate: f32) -> Self {
		Self {
			sample_rate,
			freq: 600.0,
			volume: 0.5,
			key_down: false,
			seq: Vec::new(),
			idx: 0,
			remaining: 0,
			playing: false,
			paused: false,
			phase: 0.0,
			env: 0.0,
			ramp_step: 1.0 / (RAMP_MS / 1000.0 * sample_rate).max(1.0),
		}
	}

	/// Advance the playback sequence by one sample; returns whether the tone is gated on.
	fn next_gate(&mut self) -> bool {
		if !self.playing || self.paused {
			return false;
		}
		loop {
			let Some(seg) = self.seq.get(self.idx).copied() else {
				self.playing = false;
				return false;
			};
			if self.remaining == 0 {
				self.idx += 1;
				if let Some(next) = self.seq.get(self.idx) {
					self.remaining = next.samples;
				}
				continue;
			}
			self.remaining -= 1;
			return seg.on;
		}
	}

	fn next_sample(&mut self) -> f32 {
		let seq_gate = self.next_gate();
		let gate = self.key_down || seq_gate;
		if gate {
			self.env = (self.env + self.ramp_step).min(1.0);
		} else {
			self.env = (self.env - self.ramp_step).max(0.0);
		}
		if self.env <= 0.0 {
			// Keep phase continuous anyway so a new tone always starts cleanly.
			self.advance_phase();
			return 0.0;
		}
		let shaped = 0.5 - 0.5 * (PI * self.env).cos();
		let s = (self.phase * TAU).sin() * shaped * self.volume;
		self.advance_phase();
		s
	}

	fn advance_phase(&mut self) {
		self.phase += self.freq / self.sample_rate;
		if self.phase >= 1.0 {
			self.phase -= 1.0;
		}
	}
}

pub struct Audio {
	synth: Arc<Mutex<Synth>>,
	sample_rate: f32,
	_stream: cpal::Stream,
}

impl Audio {
	pub fn new() -> Result<Self, String> {
		let host = cpal::default_host();
		let device = host
			.default_output_device()
			.ok_or_else(|| "no audio output device found".to_string())?;
		let supported = device
			.default_output_config()
			.map_err(|e| format!("audio config: {e}"))?;
		let format = supported.sample_format();
		let config: cpal::StreamConfig = supported.into();
		let sample_rate = config.sample_rate.0 as f32;
		let channels = config.channels as usize;
		let synth = Arc::new(Mutex::new(Synth::new(sample_rate)));

		let s = synth.clone();
		let stream = match format {
			SampleFormat::F32 => build::<f32>(&device, &config, channels, s),
			SampleFormat::F64 => build::<f64>(&device, &config, channels, s),
			SampleFormat::I16 => build::<i16>(&device, &config, channels, s),
			SampleFormat::U16 => build::<u16>(&device, &config, channels, s),
			SampleFormat::I32 => build::<i32>(&device, &config, channels, s),
			SampleFormat::U32 => build::<u32>(&device, &config, channels, s),
			SampleFormat::I8 => build::<i8>(&device, &config, channels, s),
			SampleFormat::U8 => build::<u8>(&device, &config, channels, s),
			other => Err(format!("unsupported sample format {other:?}")),
		}?;
		stream.play().map_err(|e| format!("audio start: {e}"))?;

		Ok(Self {
			synth,
			sample_rate,
			_stream: stream,
		})
	}

	fn lock(&self) -> MutexGuard<'_, Synth> {
		self.synth.lock().unwrap_or_else(|e| e.into_inner())
	}

	pub fn set_tone(&self, freq: f32, volume: f32) {
		let mut s = self.lock();
		s.freq = freq;
		s.volume = volume.clamp(0.0, 1.0);
	}

	fn to_samples(&self, ms: f32) -> u64 {
		(ms / 1000.0 * self.sample_rate).round().max(0.0) as u64
	}

	/// Start playing a sequence from the beginning (replacing any current one).
	pub fn play(&self, elements: &[Element]) {
		self.start(elements, LEAD_IN_MS);
	}

	/// Play one element right away, or queue it (after `gap_ms` of silence)
	/// behind whatever is still sounding, so fast taps don't cut each other off.
	pub fn tap(&self, element: Element, gap_ms: f32) {
		let mut s = self.lock();
		if s.playing {
			let gap = self.to_samples(gap_ms);
			let on = self.to_samples(element.ms);
			s.seq.push(Segment {
				on: false,
				samples: gap,
			});
			s.seq.push(Segment {
				on: true,
				samples: on,
			});
			return;
		}
		drop(s);
		self.start(&[element], 0.0);
	}

	fn start(&self, elements: &[Element], lead_in_ms: f32) {
		let mut seq = Vec::with_capacity(elements.len() + 1);
		seq.push(Segment {
			on: false,
			samples: self.to_samples(lead_in_ms),
		});
		seq.extend(elements.iter().map(|e| Segment {
			on: e.on,
			samples: self.to_samples(e.ms),
		}));

		let mut s = self.lock();
		s.remaining = seq[0].samples;
		s.seq = seq;
		s.idx = 0;
		s.paused = false;
		s.playing = !elements.is_empty();
	}

	pub fn stop(&self) {
		let mut s = self.lock();
		s.playing = false;
		s.paused = false;
		s.seq.clear();
	}
}

fn build<T>(
	device: &cpal::Device,
	config: &cpal::StreamConfig,
	channels: usize,
	synth: Arc<Mutex<Synth>>,
) -> Result<cpal::Stream, String>
where
	T: SizedSample + FromSample<f32>,
{
	device
		.build_output_stream(
			config,
			move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
				let mut s = synth.lock().unwrap_or_else(|e| e.into_inner());
				for frame in data.chunks_mut(channels) {
					let v = T::from_sample(s.next_sample());
					for out in frame.iter_mut() {
						*out = v;
					}
				}
			},
			|err| eprintln!("audio stream error: {err}"),
			None,
		)
		.map_err(|e| format!("audio stream: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn envelope_ramps_instead_of_jumping() {
		let mut s = Synth::new(48_000.0);
		s.volume = 1.0;
		s.key_down = true;
		let ramp = (RAMP_MS / 1000.0 * 48_000.0) as usize; // 240 samples
		let first: Vec<f32> = (0..ramp / 4).map(|_| s.next_sample().abs()).collect();
		assert!(first.iter().all(|v| *v < 0.2), "attack too abrupt");
		for _ in 0..ramp {
			s.next_sample();
		}
		assert!(s.env >= 1.0);
		s.key_down = false;
		let release: Vec<f32> = (0..ramp + 2).map(|_| s.next_sample()).collect();
		assert_eq!(*release.last().unwrap(), 0.0);
		// No sample-to-sample jump bigger than a full-scale sine at 600 Hz allows.
		let max_step = TAU * 600.0 / 48_000.0 + 0.02;
		assert!(release.windows(2).all(|w| (w[1] - w[0]).abs() <= max_step));
	}

	#[test]
	fn sequence_plays_and_finishes() {
		let mut s = Synth::new(1000.0); // 1 sample per ms
		s.seq = vec![
			Segment {
				on: true,
				samples: 3,
			},
			Segment {
				on: false,
				samples: 2,
			},
			Segment {
				on: true,
				samples: 1,
			},
		];
		s.remaining = 3;
		s.playing = true;
		let gates: Vec<bool> = (0..8).map(|_| s.next_gate()).collect();
		assert_eq!(
			gates,
			vec![true, true, true, false, false, true, false, false]
		);
		assert!(!s.playing);
	}
}
