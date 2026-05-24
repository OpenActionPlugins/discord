use crate::client::discord_client;
use crate::utils::VoiceSettingsWrapper;

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::Ordering::Relaxed;

use discord_ipc_rust::models::send::commands::{SentCommand, SetVoiceSettingsArgs};
use discord_ipc_rust::models::shared::voice::{VoiceSettingsInput, VoiceSettingsOutput};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use tokio::sync::RwLock;

mod set_audio_device;
mod volume_control;
pub use set_audio_device::*;
pub use volume_control::*;

pub fn voice_input_settings() -> &'static RwLock<Option<VoiceSettingsWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<VoiceSettingsWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

pub fn voice_output_settings() -> &'static RwLock<Option<VoiceSettingsWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<VoiceSettingsWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

// Centralize the voice settings RPC call and Stream Deck feedback logic.
async fn update_voice_setting(
	instance: &Instance,
	args: SetVoiceSettingsArgs,
	next_state: usize,
) -> OpenActionResult<()> {
	// Take the shared IPC client so we can send the voice update command.
	let mut client_lock = discord_client().write().await;
	let Some(client) = client_lock.as_mut() else {
		log::error!("Discord client not initialized");
		instance.show_alert().await?;
		return Ok(());
	};

	// Send the RPC and update the Stream Deck feedback depending on the result.
	match client
		.emit_command(&SentCommand::SetVoiceSettings(args))
		.await
	{
		Ok(_) => {
			// Reflect the new voice state on the button.
			instance.set_state(next_state as u16).await?;
		}
		Err(e) => {
			log::error!("Failed to update voice state: {}", e);
			instance.show_alert().await?;
		}
	}

	Ok(())
}

pub struct ToggleMuteAction;
#[async_trait]
impl Action for ToggleMuteAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglemute";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let current_state = instance.current_state_index.load(Relaxed);
		let new_mute = current_state == 0;

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

pub struct ToggleDeafenAction;
#[async_trait]
impl Action for ToggleDeafenAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.toggledeafen";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let current_state = instance.current_state_index.load(Relaxed);
		let new_deaf = current_state == 0;

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

pub struct PushToMuteAction;
#[async_trait]
impl Action for PushToMuteAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.pushtomute";
	type Settings = HashMap<String, String>;

	async fn key_down(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_voice_setting(
			instance,
			SetVoiceSettingsArgs {
				mute: Some(true),
				..Default::default()
			},
			1,
		)
		.await
	}

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_voice_setting(
			instance,
			SetVoiceSettingsArgs {
				mute: Some(false),
				..Default::default()
			},
			0,
		)
		.await
	}
}

pub struct PushToTalkAction;
#[async_trait]
impl Action for PushToTalkAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.pushtotalk";
	type Settings = HashMap<String, String>;

	async fn key_down(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_voice_setting(
			instance,
			SetVoiceSettingsArgs {
				mute: Some(false),
				..Default::default()
			},
			1,
		)
		.await
	}

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_voice_setting(
			instance,
			SetVoiceSettingsArgs {
				mute: Some(true),
				..Default::default()
			},
			0,
		)
		.await
	}
}

pub(super) fn audio_device_setting_args(
	wrapper: VoiceSettingsWrapper,
	device_type: &crate::utils::VoiceDeviceType,
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

pub(super) async fn get_current_voice_settings(
	instance: &Instance,
	device_type: &crate::utils::VoiceDeviceType,
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
		log::error!(
			"No voice settings found for type {:?}, likely not in a voice channel",
			device_type
		);
		instance.show_alert().await?;
		return Ok(None);
	}

	Ok(voice_settings)
}

pub(super) async fn with_current_voice_settings<R>(
	instance: &Instance,
	device_type: &crate::utils::VoiceDeviceType,
	updater: impl FnOnce(&mut VoiceSettingsWrapper) -> R,
) -> OpenActionResult<Option<R>> {
	let mut voice_setting_write_lock = if device_type.is_input() {
		voice_input_settings()
	} else {
		voice_output_settings()
	}
	.write()
	.await;

	let Some(voice_setting) = voice_setting_write_lock.as_mut() else {
		drop(voice_setting_write_lock); // Drop the lock before show_alert()

		log::error!(
			"No voice setting found for type {:?}, cannot update",
			device_type
		);
		instance.show_alert().await?;
		return Ok(None);
	};

	Ok(Some(updater(voice_setting)))
}
