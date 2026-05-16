use discord_ipc_rust::models::send::commands::SetVoiceSettingsArgs;
use discord_ipc_rust::models::shared::voice::{VoiceSettingsInput, VoiceSettingsOutput};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

use super::update_voice_setting;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct AudioSettingWrapper {
	pub device_id: String,
	pub volume: f32,
}

impl From<VoiceSettingsInput> for AudioSettingWrapper {
	fn from(input: VoiceSettingsInput) -> Self {
		Self {
			device_id: input.device_id,
			volume: input.volume,
		}
	}
}

impl From<VoiceSettingsOutput> for AudioSettingWrapper {
	fn from(output: VoiceSettingsOutput) -> Self {
		Self {
			device_id: output.device_id,
			volume: output.volume,
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct VolumeChangeSettings {
	pub pressing: bool,
	pub toggle: bool,

	pub data: Option<AudioSettingWrapper>,
}

pub struct InputVolumeChangeAction;
#[async_trait]
impl Action for InputVolumeChangeAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.inputvolumechange";
	type Settings = VolumeChangeSettings;

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let mut new_settings = settings.clone();
		new_settings.pressing = false;

		instance.set_settings(&new_settings).await?;
		Ok(())
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		if let Some(input) = &settings.data {
			let current_volume = input.volume;

			let delta = (ticks as f32) * 6.0;
			let new_volume = (current_volume + delta).clamp(0.0, 100.0);
			let mut new_settings = settings.clone();
			new_settings.toggle = true;
			new_settings.data = Some(AudioSettingWrapper {
				device_id: input.device_id.clone(),
				volume: new_volume,
			});
			instance.set_settings(&new_settings).await?;

			if (new_volume - current_volume).abs() > 0f32 {
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs {
						input: Some(VoiceSettingsInput {
							device_id: input.device_id.clone(),
							volume: new_volume,
							available_devices: Vec::new(),
						}),
						..Default::default()
					},
					1,
				)
				.await
			} else {
				Ok(())
			}
		} else {
			Ok(())
		}
	}

	async fn dial_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let mut new_settings = settings.clone();
		let new_mute = !settings.toggle;

		new_settings.toggle = new_mute;
		instance.set_settings(&new_settings).await?;

		update_voice_setting(
			instance,
			SetVoiceSettingsArgs {
				mute: Some(new_mute),
				..Default::default()
			},
			if new_mute { 1 } else { 0 },
		)
		.await
	}
}

pub struct OutputVolumeChangeAction;
#[async_trait]
impl Action for OutputVolumeChangeAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.outputvolumechange";
	type Settings = VolumeChangeSettings;

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let mut new_settings = settings.clone();
		new_settings.pressing = false;

		instance.set_settings(&new_settings).await?;
		Ok(())
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		if let Some(output) = &settings.data {
			let current_volume = output.volume;
			let delta = (ticks as f32) * 6.0;
			let new_volume = (current_volume + delta).clamp(0.0, 200.0);

			let mut new_settings = settings.clone();
			new_settings.toggle = true;
			new_settings.data = Some(AudioSettingWrapper {
				volume: new_volume,
				device_id: output.device_id.clone(),
			});
			instance.set_settings(&new_settings).await?;

			if (new_volume - current_volume).abs() > 0f32 {
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs {
						output: Some(VoiceSettingsOutput {
							device_id: output.device_id.clone(),
							volume: new_volume,
							available_devices: Vec::new(),
						}),
						..Default::default()
					},
					1,
				)
				.await
			} else {
				Ok(())
			}
		} else {
			Ok(())
		}
	}

	async fn dial_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let mut new_settings = settings.clone();
		let new_deaf = !settings.toggle;

		new_settings.toggle = new_deaf;
		instance.set_settings(&new_settings).await?;

		update_voice_setting(
			instance,
			SetVoiceSettingsArgs {
				deaf: Some(new_deaf),
				..Default::default()
			},
			if new_deaf { 1 } else { 0 },
		)
		.await
	}
}
