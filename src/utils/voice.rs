use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct VoiceSettingsWrapper {
	pub device_id: String,
	pub volume: f32,
	pub enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub enum VoiceDeviceType {
	Input,
	#[default]
	Output,
}

impl VoiceDeviceType {
	pub fn is_input(&self) -> bool {
		matches!(self, Self::Input)
	}

	pub fn max_volume(&self) -> f32 {
		if self.is_input() { 100.0 } else { 200.0 }
	}

	pub fn to_linear(&self, discord_vol: f32) -> f32 {
		if discord_vol <= 0.0 {
			return 0.0;
		}

		let linear_vol = if discord_vol >= 100.0 {
			// 100 + 100 * (ln(discord_vol) - 4.60517019) / 0.69077554
			(100.0 + 100.0 * (discord_vol.ln() - 4.60517019) / 0.69077555).round().clamp(0.0, self.max_volume())
		} else {
			(100.0 * (discord_vol / 100.0).powf(1.0 / 2.8)).clamp(0.0, 100.0)
		};

		linear_vol
	}

	pub fn to_discord(&self, linear_vol: f32) -> f32 {
		if linear_vol <= 0.0 {
			return 0.0;
		}

		let discord_vol = if linear_vol > 100.0 {
		    let x = linear_vol.clamp(100.0, self.max_volume());

			// 100 * (1.99526234 ** ((x - 100) / 100))
			100.0 * (1.99526234_f32.powf((x - 100.0) / 100.0))
		} else {
			(100.0 * (linear_vol / 100.0).powf(2.8)).clamp(0.0, 100.0)
		};

		discord_vol
	}
}
