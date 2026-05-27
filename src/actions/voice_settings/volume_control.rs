use super::audio_device_utils::{AudioDeviceType, AudioDeviceWrapper, get_audio_device_settings};
use super::update_voice_setting;

use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub enum VolumeControlActionType {
	#[default]
	Increase,
	Decrease,
	Set,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct VolumeControlSettings {
	pub device_type: AudioDeviceType,
	pub action_type: VolumeControlActionType,
	pub step_size: u8,
	pub set_volume: u8,
}

impl Default for VolumeControlSettings {
	fn default() -> Self {
		Self {
			device_type: AudioDeviceType::Input,
			action_type: VolumeControlActionType::default(),
			step_size: 5,
			set_volume: 100,
		}
	}
}

async fn adjust_volume(
	instance: &Instance,
	device_type: &AudioDeviceType,
	value: f32,
	set: bool,
) -> OpenActionResult<()> {
	let Some(device_settings) = get_audio_device_settings(device_type).await else {
		log::error!(
			"Failed to obtain voice settings for {:?} device",
			device_type
		);
		instance.show_alert().await?;
		return Ok(());
	};

	let current_linear = device_type.to_linear(device_settings.volume);
	let new_linear =
		if set { value } else { current_linear + value }.clamp(0.0, device_type.max_volume());

	if new_linear == current_linear {
		return Ok(());
	}

	let updated_settings = AudioDeviceWrapper {
		volume: device_type.to_discord(new_linear),
		..device_settings
	};

	update_voice_setting(instance, updated_settings.into(), 0).await
}

pub struct VolumeControlAction;
#[async_trait]
impl Action for VolumeControlAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.volumecontrol";
	type Settings = VolumeControlSettings;

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let value = match settings.action_type {
			VolumeControlActionType::Increase => settings.step_size as f32,
			VolumeControlActionType::Decrease => -(settings.step_size as f32),
			VolumeControlActionType::Set => settings.set_volume as f32,
		};

		adjust_volume(
			instance,
			&settings.device_type,
			value,
			matches!(settings.action_type, VolumeControlActionType::Set),
		)
		.await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let delta = (settings.step_size as f32) * ticks as f32;

		adjust_volume(instance, &settings.device_type, delta, false).await
	}
}
