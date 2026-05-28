use super::audio_device_utils::{AudioDeviceType, AudioDeviceWrapper, get_audio_device_settings};
use super::update_voice_setting;

use discord_ipc_rust::models::shared::voice::VoiceAvailableDevice;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub enum AudioDeviceTarget {
	#[default]
	Input,
	Output,
	Both,
}

impl AudioDeviceTarget {
	fn requires_input(&self) -> bool {
		matches!(self, Self::Input | Self::Both)
	}

	fn requires_output(&self) -> bool {
		matches!(self, Self::Output | Self::Both)
	}
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct SetAudioDeviceSettings {
	pub target: AudioDeviceTarget,
	pub input_device_id: Option<String>,
	pub output_device_id: Option<String>,
}

async fn update_device(
	instance: &Instance,
	device_type: &AudioDeviceType,
	device_id: String,
) -> OpenActionResult<()> {
	let Some(current) = get_audio_device_settings(device_type).await else {
		log::error!(
			"Failed to obtain voice settings for {:?} device",
			device_type
		);
		instance.show_alert().await?;
		return Ok(());
	};

	let device_available = current.available_devices.iter().any(|d| d.id == device_id);
	if !device_available {
		log::error!(
			"Selected device '{}' not available for {:?}",
			device_id,
			device_type
		);
		instance.show_alert().await?;
		return Ok(());
	}

	if current.device_id == device_id {
		return Ok(());
	}

	let updated_settings = AudioDeviceWrapper {
		device_id,
		..current
	};

	update_voice_setting(instance, updated_settings.into(), 0).await
}

pub async fn send_avaliable_devices_to_pi(
	instance: &Instance,
) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct Payload {
		input_devices: Vec<VoiceAvailableDevice>,
		output_devices: Vec<VoiceAvailableDevice>,
	}

	async fn fetch_list(
		device_type: &AudioDeviceType,
	) -> Vec<VoiceAvailableDevice> {
		get_audio_device_settings(device_type)
			.await
			.map(|s| s.available_devices)
			.unwrap_or_default()
	}

	instance
		.send_to_property_inspector(Payload {
			input_devices: fetch_list(&AudioDeviceType::Input)
				.await,
			output_devices: fetch_list(&AudioDeviceType::Output)
				.await,
		})
		.await
}

pub struct SetAudioDeviceAction;
#[async_trait]
impl Action for SetAudioDeviceAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.setaudiodevice";
	type Settings = SetAudioDeviceSettings;

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_avaliable_devices_to_pi(instance).await
	}

	async fn will_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_avaliable_devices_to_pi(instance).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let pairs = [
			(
				settings.target.requires_input(),
				&AudioDeviceType::Input,
				&settings.input_device_id,
			),
			(
				settings.target.requires_output(),
				&AudioDeviceType::Output,
				&settings.output_device_id,
			),
		];

		for (required, device_type, device_id) in pairs {
			if !required {
				continue;
			}

			let Some(id) = device_id.as_deref().filter(|id| !id.is_empty()) else {
				log::error!("No device ID provided for {:?} device", device_type);
				instance.show_alert().await?;
				return Ok(());
			};
			let id = id.to_string();

			update_device(instance, device_type, id).await?;
		}

		Ok(())
	}
}
