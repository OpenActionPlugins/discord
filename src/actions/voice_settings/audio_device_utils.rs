use crate::image_utils;

use std::{collections::HashMap, sync::OnceLock};

use discord_ipc_rust::models::{
	receive::events::VoiceStateData,
	send::commands::SetVoiceSettingsArgs,
	shared::voice::{VoiceAvailableDevice, VoiceSettingsInput, VoiceSettingsOutput},
};
use image::Rgba;
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
	pub available_devices: Vec<VoiceAvailableDevice>,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct UserVoiceSettings {
	pub nick: String,
	pub avatar: Option<String>,
	pub volume: f32,
	pub mute: bool,
	pub self_mute: bool,
	pub self_deaf: bool,
	pub server_mute: bool,
	pub server_deaf: bool,
	pub speaking: bool,
}

impl From<VoiceStateData> for UserVoiceSettings {
	fn from(value: VoiceStateData) -> Self {
		Self {
			nick: value.nick,
			avatar: value.user.and_then(|user| user.avatar),
			volume: value.volume,
			mute: value.mute,
			self_mute: value.state.self_mute,
			self_deaf: value.state.self_deaf,
			server_mute: value.state.mute,
			server_deaf: value.state.deaf,
			// Not part of a voice state payload; callers should carry the previous value over.
			speaking: false,
		}
	}
}

// Builds a Discord CDN URL for a user's avatar, falling back to their default avatar
// (computed from their ID) when they have no custom avatar set. Always requests a static PNG,
// since we decode and recomposite the image locally (see `avatar_data_uri`) regardless of
// whether the source avatar is animated.
pub fn avatar_url(user_id: &str, avatar_hash: Option<&str>) -> String {
	match avatar_hash {
		Some(hash) => format!("https://cdn.discordapp.com/avatars/{user_id}/{hash}.png?size=64"),
		None => {
			let index = user_id.parse::<u64>().map(|id| (id >> 22) % 6).unwrap_or(0);
			format!("https://cdn.discordapp.com/embed/avatars/{index}.png")
		}
	}
}

const AVATAR_SIZE: u32 = 96;
const SPEAKING_BORDER_WIDTH: f32 = 6.0;
const SPEAKING_BORDER_COLOR: Rgba<u8> = Rgba([87, 242, 135, 255]); // Discord "green"

// Downloads (or reuses a cached copy of) a user's avatar, crops it to a circle (with a green
// ring when they're currently speaking), and returns it as a base64 data URI suitable for
// SetImage/SetFeedback.
pub async fn avatar_data_uri(
	user_id: &str,
	avatar_hash: Option<&str>,
	speaking: bool,
) -> Option<String> {
	let url = avatar_url(user_id, avatar_hash);
	let bytes = image_utils::fetch_bytes(&url).await?;
	let border = speaking.then_some((SPEAKING_BORDER_WIDTH, SPEAKING_BORDER_COLOR));
	let png_bytes = image_utils::circular_png(&bytes, AVATAR_SIZE, border)?;
	Some(image_utils::to_data_uri(&png_bytes))
}

pub fn audio_input_settings() -> &'static RwLock<Option<AudioDeviceWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<AudioDeviceWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

pub fn audio_output_settings() -> &'static RwLock<Option<AudioDeviceWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<AudioDeviceWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

pub fn user_voice_settings_map() -> &'static RwLock<HashMap<String, UserVoiceSettings>> {
	static MAP: OnceLock<RwLock<HashMap<String, UserVoiceSettings>>> = OnceLock::new();
	MAP.get_or_init(Default::default)
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
