use std::sync::OnceLock;

use discord_ipc_rust::models::{
	send::commands::SetVoiceSettingsArgs,
	shared::voice::{VoiceSettingsInput, VoiceSettingsOutput},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AudioDeviceType {
	Input,
	Output,
}

impl AudioDeviceType {
	pub fn max_volume(&self) -> f32 {
		match self {
			Self::Input => 100.0,
			Self::Output => 200.0,
		}
	}

	pub fn to_linear(&self, discord_vol: f32) -> f32 {
		if discord_vol <= 0.0 {
			return 0.0;
		}

		if discord_vol >= 100.0 {
			(100.0 + 100.0 * (discord_vol.ln() - 4.605_170_2) / 0.690_775_6)
				.round()
				.clamp(0.0, self.max_volume())
		} else {
			(100.0 * (discord_vol / 100.0).powf(1.0 / 2.8)).clamp(0.0, 100.0)
		}
	}

	pub fn to_discord(&self, linear_vol: f32) -> f32 {
		if linear_vol <= 0.0 {
			return 0.0;
		}

		if linear_vol > 100.0 {
			let x = linear_vol.clamp(100.0, self.max_volume());
			100.0 * (1.995_262_4_f32.powf((x - 100.0) / 100.0))
		} else {
			(100.0 * (linear_vol / 100.0).powf(2.8)).clamp(0.0, 100.0)
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioDeviceWrapper {
	pub device_type: AudioDeviceType,
	pub device_id: String,
	pub volume: f32,
}

impl From<AudioDeviceWrapper> for SetVoiceSettingsArgs {
	fn from(value: AudioDeviceWrapper) -> Self {
		match value.device_type {
			AudioDeviceType::Input => SetVoiceSettingsArgs {
				input: Some(VoiceSettingsInput {
					device_id: value.device_id.clone(),
					volume: value.volume,
					available_devices: Vec::new(),
				}),
				..Default::default()
			},
			AudioDeviceType::Output => SetVoiceSettingsArgs {
				output: Some(VoiceSettingsOutput {
					device_id: value.device_id.clone(),
					volume: value.volume,
					available_devices: Vec::new(),
				}),
				..Default::default()
			},
		}
	}
}

pub fn audio_input_settings() -> &'static RwLock<Option<AudioDeviceWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<AudioDeviceWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

pub fn audio_output_settings() -> &'static RwLock<Option<AudioDeviceWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<AudioDeviceWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

pub async fn get_audio_device_settings(
	device_type: &AudioDeviceType,
) -> Option<AudioDeviceWrapper> {
	match device_type {
		AudioDeviceType::Input => audio_input_settings(),
		AudioDeviceType::Output => audio_output_settings(),
	}
	.read()
	.await
	.clone()
}
