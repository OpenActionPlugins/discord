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

pub struct SetAudioDeviceAction;
#[async_trait]
impl Action for SetAudioDeviceAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.setaudiodevice";
	type Settings = SetAudioDeviceSettings;

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_avaliable_devices_to_pi(instance, settings).await
	}

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_avaliable_devices_to_pi(instance, settings).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let input_id = if settings.target.requires_input() {
			match require_device_id(
				instance,
				&AudioDeviceType::Input,
				settings.input_device_id.as_deref(),
			)
			.await?
			{
				Some(device_id) => Some(device_id),
				None => return Ok(()),
			}
		} else {
			None
		};
		let output_id = if settings.target.requires_output() {
			match require_device_id(
				instance,
				&AudioDeviceType::Output,
				settings.output_device_id.as_deref(),
			)
			.await?
			{
				Some(device_id) => Some(device_id),
				None => return Ok(()),
			}
		} else {
			None
		};

		if let Some(input_id) = input_id {
			apply_device_update(instance, &AudioDeviceType::Input, input_id).await?;
		}
		if let Some(output_id) = output_id {
			apply_device_update(instance, &AudioDeviceType::Output, output_id).await?;
		}

		Ok(())
	}
}

async fn require_device_id(
	instance: &Instance,
	device_type: &AudioDeviceType,
	device_id: Option<&str>,
) -> OpenActionResult<Option<String>> {
	let Some(device_id) = device_id.map(str::trim).filter(|id| !id.is_empty()) else {
		log::error!("No {:?} device selected", device_type);
		instance.show_alert().await?;
		return Ok(None);
	};

	Ok(Some(device_id.to_owned()))
}

fn check_device_avaliable(current: &AudioDeviceWrapper, device_id: &str) -> bool {
	if current.available_devices.is_empty() {
		return false;
	}

	current
		.available_devices
		.iter()
		.any(|device| device.id == device_id)
}

async fn apply_device_update(
	instance: &Instance,
	device_type: &AudioDeviceType,
	device_id: String,
) -> OpenActionResult<()> {
	let Some(voice_settings) = get_audio_device_settings(device_type).await else {
		log::error!(
			"Failed to obtain voice settings for {:?} device",
			device_type
		);
		instance.show_alert().await?;
		return Ok(());
	};

	if !check_device_avaliable(&voice_settings, &device_id) {
		log::error!(
			"Selected device '{}' not available for {:?}",
			device_id,
			device_type
		);

		instance.show_alert().await?;
		return Ok(());
	}

	if voice_settings.device_id == device_id {
		return Ok(());
	}

	let updated_voice_settings = AudioDeviceWrapper {
		device_id,
		..voice_settings
	};

	update_voice_setting(instance, updated_voice_settings.into(), 0).await
}

async fn send_avaliable_devices_to_pi(
	instance: &Instance,
	settings: &SetAudioDeviceSettings,
) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct Payload {
		input_devices: Vec<VoiceAvailableDevice>,
		output_devices: Vec<VoiceAvailableDevice>,
	}

	let input_devices = if settings.target.requires_input() {
		get_audio_device_settings(&AudioDeviceType::Input)
			.await
			.map(|settings| settings.available_devices)
			.unwrap_or_default()
	} else {
		Vec::new()
	};

	let output_devices = if settings.target.requires_output() {
		get_audio_device_settings(&AudioDeviceType::Output)
			.await
			.map(|settings| settings.available_devices)
			.unwrap_or_default()
	} else {
		Vec::new()
	};

	instance
		.send_to_property_inspector(Payload {
			input_devices,
			output_devices,
		})
		.await
}
