use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct VoiceSettingsWrapper {
	pub device_id: String,
	pub volume: f32,

	pub enable: bool,
}
