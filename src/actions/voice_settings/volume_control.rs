use super::{update_voice_setting, voice_input_settings, voice_output_settings};
use crate::utils::VoiceSettingsWrapper;

use discord_ipc_rust::models::send::commands::SetVoiceSettingsArgs;
use discord_ipc_rust::models::shared::voice::{VoiceSettingsInput, VoiceSettingsOutput};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub enum AudioType {
	Input,
	#[default]
	Output,
}

impl AudioType {
	fn is_input(&self) -> bool {
		matches!(self, Self::Input)
	}

	fn max_volume(&self) -> f32 {
		if self.is_input() { 100.0 } else { 200.0 }
	}
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub enum StepDirection {
	#[default]
	Increase,
	Decrease,
}

impl StepDirection {
	fn multiplier(&self) -> f32 {
		match self {
			Self::Increase => 1.0,
			Self::Decrease => -1.0,
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct VolumeControlSettings {
	pub pressing: bool,

	pub r#type: AudioType,
	pub step_direction: StepDirection,
	pub steps: u8,
}

impl Default for VolumeControlSettings {
	fn default() -> Self {
		Self {
			pressing: false,
			r#type: AudioType::default(),
			step_direction: StepDirection::default(),
			steps: 2,
		}
	}
}

pub struct VolumeControlAction;
#[async_trait]
impl Action for VolumeControlAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.volumecontrol";
	type Settings = VolumeControlSettings;

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		if let Some(voice_settings) = get_current_voice_settings(instance, &settings.r#type).await?
		{
			instance
				.set_state(if voice_settings.enable { 0 } else { 1 })
				.await?;
		}

		Ok(())
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let mut new_settings = settings.clone();
		new_settings.pressing = false;

		instance.set_settings(&new_settings).await?;
		Ok(())
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let mut new_settings = settings.clone();
		new_settings.pressing = true;
		instance.set_settings(&new_settings).await?;

		// TODO: allow holding key to continuously adjust volume instead of just once on key down

		let delta = (settings.steps as f32) * settings.step_direction.multiplier();
		adjust_volume(instance, settings, delta).await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let direction = ticks.signum() as f32;
		let delta = (settings.steps as f32) * direction * settings.step_direction.multiplier();

		adjust_volume(instance, settings, delta).await
	}

	async fn dial_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let Some(new_toggle) =
			with_current_voice_settings(instance, &settings.r#type, |voice_settings| {
				let new_toggle = voice_settings.enable;
				voice_settings.enable = !new_toggle;
				new_toggle
			})
			.await?
		else {
			return Ok(());
		};

		let args = if settings.r#type.is_input() {
			SetVoiceSettingsArgs {
				mute: Some(new_toggle),
				..Default::default()
			}
		} else {
			SetVoiceSettingsArgs {
				deaf: Some(new_toggle),
				..Default::default()
			}
		};

		update_voice_setting(instance, args, if new_toggle { 1 } else { 0 }).await
	}
}

async fn get_current_voice_settings(
	instance: &Instance,
	audio_type: &AudioType,
) -> OpenActionResult<Option<VoiceSettingsWrapper>> {
	let voice_setting = if audio_type.is_input() {
		voice_input_settings()
	} else {
		voice_output_settings()
	}
	.read()
	.await;

	let Some(voice_setting) = voice_setting.as_ref() else {
		log::error!(
			"No voice setting found for type {:?}, cannot get",
			audio_type
		);
		instance.show_alert().await?;
		return Ok(None);
	};

	Ok(Some(voice_setting.clone()))
}

async fn with_current_voice_settings<R>(
	instance: &Instance,
	audio_type: &AudioType,
	updater: impl FnOnce(&mut VoiceSettingsWrapper) -> R,
) -> OpenActionResult<Option<R>> {
	let mut voice_setting = if audio_type.is_input() {
		voice_input_settings()
	} else {
		voice_output_settings()
	}
	.write()
	.await;

	let Some(voice_setting) = voice_setting.as_mut() else {
		log::error!(
			"No voice setting found for type {:?}, cannot update",
			audio_type
		);
		instance.show_alert().await?;
		return Ok(None);
	};

	Ok(Some(updater(voice_setting)))
}

fn volume_args(wrapper: VoiceSettingsWrapper, audio_type: &AudioType) -> SetVoiceSettingsArgs {
	if audio_type.is_input() {
		SetVoiceSettingsArgs {
			input: Some(VoiceSettingsInput {
				device_id: wrapper.device_id,
				volume: wrapper.volume,
				available_devices: Vec::new(),
			}),
			..Default::default()
		}
	} else {
		SetVoiceSettingsArgs {
			output: Some(VoiceSettingsOutput {
				device_id: wrapper.device_id,
				volume: wrapper.volume,
				available_devices: Vec::new(),
			}),
			..Default::default()
		}
	}
}

async fn adjust_volume(
	instance: &Instance,
	settings: &VolumeControlSettings,
	delta: f32,
) -> OpenActionResult<()> {
	enum VolumeAdjustOutcome {
		Alert,
		Success {
			args: SetVoiceSettingsArgs,
			enable: bool,
		},
	}

	let Some(result) = with_current_voice_settings(instance, &settings.r#type, |voice_settings| {
		let current_volume = voice_settings.volume;
		let new_volume = (current_volume + delta).clamp(0.0, settings.r#type.max_volume());

		if current_volume == new_volume {
			return VolumeAdjustOutcome::Alert;
		}

		voice_settings.volume = new_volume;

		let args = volume_args(
		    voice_settings.clone(),
			&settings.r#type,
		);
		VolumeAdjustOutcome::Success {
			args,
			enable: voice_settings.enable,
		}
	})
	.await?
	else {
		return Ok(());
	};

	match result {
		VolumeAdjustOutcome::Alert => {
			instance.show_alert().await?;
			Ok(())
		}
		VolumeAdjustOutcome::Success { args, enable } => {
			update_voice_setting(instance, args, if enable { 0 } else { 1 }).await
		}
	}
}
