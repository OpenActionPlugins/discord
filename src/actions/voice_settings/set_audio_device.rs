use super::{audio_device_setting_args, update_voice_setting, with_current_voice_settings};
use crate::actions::get_current_voice_settings;
use crate::utils::{VoiceDeviceType, VoiceDeviceWrapper, VoiceSettingsWrapper};

use discord_ipc_rust::models::send::commands::SetVoiceSettingsArgs;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub enum SetActionDeviceActionType {
	Input,
	#[default]
	Output,
	Both,
}

impl SetActionDeviceActionType {
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
	pub r#type: SetActionDeviceActionType,
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

	async fn property_inspector_did_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_avaliable_devices_to_pi(instance, settings).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let input_id = if settings.r#type.requires_input() {
			match require_device_id(
				instance,
				VoiceDeviceType::Input,
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
		let output_id = if settings.r#type.requires_output() {
			match require_device_id(
				instance,
				VoiceDeviceType::Output,
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
			apply_device_update(instance, VoiceDeviceType::Input, input_id).await?;
		}
		if let Some(output_id) = output_id {
			apply_device_update(instance, VoiceDeviceType::Output, output_id).await?;
		}

		Ok(())
	}
}

async fn require_device_id(
	instance: &Instance,
	device_type: VoiceDeviceType,
	device_id: Option<&str>,
) -> OpenActionResult<Option<String>> {
	let Some(device_id) = device_id.map(str::trim).filter(|id| !id.is_empty()) else {
		log::error!("No {} device selected", device_type.label());
		instance.show_alert().await?;
		return Ok(None);
	};

	Ok(Some(device_id.to_owned()))
}

fn check_device_avaliable(current: &VoiceSettingsWrapper, device_id: &str) -> bool {
	if current.avaliable_devices.is_empty() {
		return false;
	}

	current
		.avaliable_devices
		.iter()
		.any(|device| device.id == device_id)
}

async fn apply_device_update(
	instance: &Instance,
	device_type: VoiceDeviceType,
	device_id: String,
) -> OpenActionResult<()> {
	enum DeviceUpdateOutcome {
		NoChange,
		UnknownDevice,
		Success { args: SetVoiceSettingsArgs },
	}

	let Some(result) = with_current_voice_settings(instance, &device_type, |current| {
		if !check_device_avaliable(current, &device_id) {
			return DeviceUpdateOutcome::UnknownDevice;
		}

		if current.device_id == device_id {
			return DeviceUpdateOutcome::NoChange;
		}

		current.device_id = device_id.to_owned();

		let args = audio_device_setting_args(current.clone(), &device_type);
		DeviceUpdateOutcome::Success { args }
	})
	.await?
	else {
		return Ok(());
	};

	match result {
		DeviceUpdateOutcome::NoChange => {
			instance.show_alert().await?;
			Ok(())
		}
		DeviceUpdateOutcome::UnknownDevice => {
			log::error!(
				"Selected device '{}' not available for {:?}",
				device_id,
				device_type
			);
			instance.show_alert().await?;
			Ok(())
		}
		DeviceUpdateOutcome::Success { args } => update_voice_setting(instance, args, 0).await,
	}
}

async fn send_avaliable_devices_to_pi(
	instance: &Instance,
	settings: &SetAudioDeviceSettings,
) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct Payload {
		input_devices: Vec<VoiceDeviceWrapper>,
		output_devices: Vec<VoiceDeviceWrapper>,
	}

	let input_devices = if settings.r#type.requires_input() {
		get_current_voice_settings(instance, &VoiceDeviceType::Input)
			.await?
			.map(|settings| settings.avaliable_devices)
			.unwrap_or_default()
	} else {
		Vec::new()
	};

	let output_devices = if settings.r#type.requires_output() {
		get_current_voice_settings(instance, &VoiceDeviceType::Output)
			.await?
			.map(|settings| settings.avaliable_devices)
			.unwrap_or_default()
	} else {
		Vec::new()
	};

	instance
		.send_to_property_inspector(Payload {
			input_devices: input_devices,
			output_devices: output_devices,
		})
		.await
}
