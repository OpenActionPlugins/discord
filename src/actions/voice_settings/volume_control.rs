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
pub enum KeypadActionType {
	#[default]
	Increase,
	Decrease,
	Set,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct VolumeControlSettings {
	pub device_type: VoiceDeviceType,
	pub keypad_action_type: KeypadActionType,
	pub step_size: u8,
	pub set_volume: u8,
}

impl Default for VolumeControlSettings {
	fn default() -> Self {
		Self {
			device_type: VoiceDeviceType::default(),
			keypad_action_type: KeypadActionType::default(),
			step_size: 5,
			set_volume: 80,
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
		update_state(instance, settings).await
	}

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_state(instance, settings).await
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
		let mut active = HOLD_ACTIVE_INSTANCE.lock().await;
		match active.as_ref() {
			Some(_) => return Ok(()),
			None => *active = Some(instance.instance_id.clone()),
		};
		drop(active);

		let (delta, set) = match settings.keypad_action_type {
			KeypadActionType::Increase => (settings.step_size as f32, false),
			KeypadActionType::Decrease => (-(settings.step_size as f32), false),
			KeypadActionType::Set => (settings.set_volume as f32, true),
		};

		use tokio::time::sleep;

		let id = instance.instance_id.clone();
		let settings_clone = settings.clone();

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
				if let Err(e) = adjust_volume(instance.as_ref(), &settings_clone, delta, set).await
				{
					log::error!("Failed to adjust volume while holding key down: {e}");
					let _ = instance.show_alert().await;
				}

				sleep(HOLD_REPEAT_INTERVAL).await;
			}
		});

		adjust_volume(instance, settings, delta, set).await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let delta = (settings.step_size as f32) * ticks as f32;

		adjust_volume(instance, settings, delta, false).await
	}

	async fn dial_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let Some(voice_settings) =
			get_current_voice_settings(instance, &settings.device_type).await?
		else {
			return Ok(());
		};
		let new_toggle = voice_settings.enable;

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

		let updated_settings = VoiceSettingsWrapper {
			enable: new_toggle,
			..voice_settings
		};

		update_voice_setting(instance, args, new_state(settings, updated_settings)).await
	}
}

async fn get_current_voice_settings(
	instance: &Instance,
	device_type: &VoiceDeviceType,
) -> OpenActionResult<Option<VoiceSettingsWrapper>> {
	// Drop the lock after cloned
	let voice_settings = {
		if device_type.is_input() {
			voice_input_settings()
		} else {
			voice_output_settings()
		}
		.read()
		.await
		.clone()
	};

	if voice_settings.is_none() {
		log::error!("No voice settings found for type {:?}", device_type);
		instance.show_alert().await?;
		return Ok(None);
	}

	Ok(voice_settings)
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
	set: bool,
) -> OpenActionResult<()> {
	let Some(voice_settings) = get_current_voice_settings(instance, &settings.device_type).await?
	else {
		return Ok(());
	};
	let current_linear = settings.device_type.to_linear(voice_settings.volume);
	let new_linear = if set { delta } else { current_linear + delta }
		.clamp(0.0, settings.device_type.max_volume());

	if new_linear == current_linear {
		return Ok(());
	}

	let updated_settings = VoiceSettingsWrapper {
		volume: settings.device_type.to_discord(new_linear),
		..voice_settings
	};
	let args = volume_args(updated_settings.clone(), &settings.device_type);

	update_voice_setting(instance, args, new_state(settings, updated_settings)).await
}

async fn update_state(
	instance: &Instance,
	settings: &VolumeControlSettings,
) -> OpenActionResult<()> {
	if let Some(voice_settings) =
		get_current_voice_settings(instance, &settings.device_type).await?
	{
		let state = new_state(settings, voice_settings.clone());
		instance.set_state(state as u16).await?;
	}

	Ok(())
}

fn new_state(settings: &VolumeControlSettings, voice_settings: VoiceSettingsWrapper) -> usize {
	let is_input = settings.device_type.is_input();
	let enabled = settings.keypad_action_type == KeypadActionType::Set || voice_settings.enable;

	match (is_input, enabled) {
		(true, true) => 0,
		(true, false) => 1,
		(false, true) => 2,
		(false, false) => 3,
	}
}
