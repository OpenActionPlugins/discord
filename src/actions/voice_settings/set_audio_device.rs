use super::update_voice_setting;
use crate::{
	audio_device_utils::{AudioDeviceType, AudioDeviceWrapper},
	client::get_audio_device_settings,
};

use discord_ipc_rust::models::shared::voice::VoiceAvailableDevice;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
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

#[derive(Serialize, Deserialize, Default)]
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
			"Failed to update {:?} device: selected device '{}' not available",
			device_type,
			device_id
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

pub async fn send_available_devices_to_pi(instance: &Instance) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct Payload {
		input_devices: Vec<VoiceAvailableDevice>,
		output_devices: Vec<VoiceAvailableDevice>,
		selected_input_device: String,
		selected_output_device: String,
	}

	async fn fetch_device_list(
		device_type: &AudioDeviceType,
	) -> (String, Vec<VoiceAvailableDevice>) {
		get_audio_device_settings(device_type)
			.await
			.map(|s| (s.device_id, s.available_devices))
			.unwrap_or_default()
	}

	let (selected_input_device, input_devices) = fetch_device_list(&AudioDeviceType::Input).await;
	let (selected_output_device, output_devices) =
		fetch_device_list(&AudioDeviceType::Output).await;

	instance
		.send_to_property_inspector(Payload {
			input_devices,
			output_devices,
			selected_input_device,
			selected_output_device,
		})
		.await
}

pub struct SetAudioDeviceAction;
#[async_trait]
impl Action for SetAudioDeviceAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.setaudiodevice";
	type Settings = SetAudioDeviceSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_available_devices_to_pi(instance).await
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_available_devices_to_pi(instance).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let targets = [
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

		for (_, device_type, device_id) in targets.iter().filter(|x| x.0) {
			let Some(id) = device_id.as_deref().filter(|id| !id.is_empty()) else {
				log::error!(
					"Failed to update {:?} device: no device ID provided",
					device_type
				);
				instance.show_alert().await?;
				return Ok(());
			};

			update_device(instance, device_type, id.to_owned()).await?;
		}

		Ok(())
	}
}
