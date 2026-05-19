use super::{update_voice_setting, voice_input_settings, voice_output_settings};
use crate::utils::{VoiceDeviceType, VoiceSettingsWrapper};

use std::sync::LazyLock;

use discord_ipc_rust::models::send::commands::SetVoiceSettingsArgs;
use discord_ipc_rust::models::shared::voice::{VoiceSettingsInput, VoiceSettingsOutput};
use openaction::{
	Action, ActionUuid, Instance, InstanceId, OpenActionResult, async_trait, visible_instances,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Duration;

const HOLD_INITIAL_DELAY: Duration = Duration::from_millis(500);
const HOLD_REPEAT_INTERVAL: Duration = Duration::from_millis(200);

static HOLD_ACTIVE_INSTANCE: LazyLock<Mutex<Option<InstanceId>>> =
	LazyLock::new(|| Mutex::new(None));

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
	pub device_type: VoiceDeviceType,
	pub step_direction: StepDirection,
	pub steps: u8,
}

impl Default for VolumeControlSettings {
	fn default() -> Self {
		Self {
			device_type: VoiceDeviceType::default(),
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
		if let Some(voice_settings) =
			get_current_voice_settings(instance, &settings.device_type).await?
		{
			instance
				.set_state(if voice_settings.enable { 0 } else { 1 })
				.await?;
		}

		Ok(())
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		clear_active_hold(&instance.instance_id).await;

		Ok(())
	}

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		clear_active_hold(&instance.instance_id).await;

		Ok(())
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let start_loop = {
			let mut active = HOLD_ACTIVE_INSTANCE.lock().await;
			// 1. If another instance is active, do nothing
			// 2. If this instance is already active, don't restart the loop
			// 3. If no instance is active, set this instance as active and start the loop
			match active.as_ref() {
				Some(active_id) if active_id != &instance.instance_id => return Ok(()),
				Some(_) => false,
				None => {
					*active = Some(instance.instance_id.clone());
					true
				}
			}
		};

		let delta = (settings.steps as f32) * settings.step_direction.multiplier();

		if start_loop {
			use tokio::time::sleep;

			let id = instance.instance_id.clone();
			let settings = settings.clone();

			tokio::spawn(async move {
				let Some(instance) = visible_instances(VolumeControlAction::UUID)
					.await
					.into_iter()
					.find(|i| i.instance_id == id)
				else {
					clear_active_hold(&id).await;
					return;
				};

				sleep(HOLD_INITIAL_DELAY).await;

				while HOLD_ACTIVE_INSTANCE.lock().await.as_ref() == Some(&id) {
					if let Err(e) = adjust_volume(instance.as_ref(), &settings, delta).await {
						log::error!("Failed to adjust volume while holding key down: {e}");
						let _ = instance.show_alert().await;
					}

					sleep(HOLD_REPEAT_INTERVAL).await;
				}
			});
		}

		adjust_volume(instance, settings, delta).await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let delta = (settings.steps as f32) * ticks as f32;

		adjust_volume(instance, settings, delta).await
	}

	async fn dial_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let Some(new_toggle) =
			with_current_voice_settings(instance, &settings.device_type, |voice_settings| {
				let new_toggle = voice_settings.enable;
				voice_settings.enable = !new_toggle;
				new_toggle
			})
			.await?
		else {
			return Ok(());
		};

		let args = if settings.device_type.is_input() {
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
	device_type: &VoiceDeviceType,
) -> OpenActionResult<Option<VoiceSettingsWrapper>> {
	let voice_setting = if device_type.is_input() {
		voice_input_settings()
	} else {
		voice_output_settings()
	}
	.read()
	.await;

	let Some(voice_setting) = voice_setting.as_ref() else {
		drop(voice_setting);

		log::error!(
			"No voice setting found for type {:?}, cannot get",
			device_type
		);
		instance.show_alert().await?;
		return Ok(None);
	};

	Ok(Some(voice_setting.clone()))
}

async fn with_current_voice_settings<R>(
	instance: &Instance,
	device_type: &VoiceDeviceType,
	updater: impl FnOnce(&mut VoiceSettingsWrapper) -> R,
) -> OpenActionResult<Option<R>> {
	let mut voice_setting = if device_type.is_input() {
		voice_input_settings()
	} else {
		voice_output_settings()
	}
	.write()
	.await;

	let Some(voice_setting) = voice_setting.as_mut() else {
		log::error!(
			"No voice setting found for type {:?}, cannot update",
			device_type
		);
		instance.show_alert().await?;
		return Ok(None);
	};

	Ok(Some(updater(voice_setting)))
}

async fn clear_active_hold(id: &InstanceId) {
	let mut active = HOLD_ACTIVE_INSTANCE.lock().await;
	if active.as_ref() == Some(id) {
		active.take();
	}
}

fn volume_args(
	wrapper: VoiceSettingsWrapper,
	device_type: &VoiceDeviceType,
) -> SetVoiceSettingsArgs {
	if device_type.is_input() {
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
		NoChange,
		Success {
			args: SetVoiceSettingsArgs,
			enable: bool,
		},
	}

	let Some(result) =
		with_current_voice_settings(instance, &settings.device_type, |voice_settings| {
			let current_linear = settings.device_type.to_linear(voice_settings.volume);
			let new_linear = (current_linear + delta).clamp(0.0, settings.device_type.max_volume());

			if (new_linear - current_linear).abs() < 0.05 {
				return VolumeAdjustOutcome::NoChange;
			}

			voice_settings.volume = settings.device_type.to_discord(new_linear);

			let args = volume_args(voice_settings.clone(), &settings.device_type);
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
		VolumeAdjustOutcome::NoChange => {
			instance.show_alert().await?;
			Ok(())
		}
		VolumeAdjustOutcome::Success { args, enable } => {
			update_voice_setting(instance, args, if enable { 0 } else { 1 }).await
		}
	}
}
