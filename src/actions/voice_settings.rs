pub mod audio_device_utils;
pub mod set_audio_device;
mod user_volume_control;
mod volume_control;

pub use set_audio_device::SetAudioDeviceAction;
pub use user_volume_control::UserVolumeControlAction;
pub use volume_control::VolumeControlAction;

use crate::client::discord_client;

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::Ordering::Relaxed;

use discord_ipc_rust::models::send::commands::{SentCommand, SetVoiceSettingsArgs};
use discord_ipc_rust::models::shared::voice::VoiceSettingsMode;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use tokio::sync::RwLock;

// Last-known voice mode from Discord, updated via RPC events.
pub fn current_voice_mode() -> &'static RwLock<Option<VoiceSettingsMode>> {
	static MODE: OnceLock<RwLock<Option<VoiceSettingsMode>>> = OnceLock::new();
	MODE.get_or_init(|| RwLock::new(None))
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

// Captured from Elgato Stream Deck on Windows (ipcproxy.log):
//   SET_VOICE_SETTINGS {"mute":false}
//   PUSH_TO_TALK {"active":true|false}
// PUSH_TO_TALK is undocumented in Discord's public RPC spec. Discord replies
// data:null then SPEAKING_START/STOP — green ring for the whole hold.
async fn push_to_talk_set(instance: &Instance, talking: bool) -> OpenActionResult<()> {
	let mut client_lock = discord_client().write().await;
	let Some(client) = client_lock.as_mut() else {
		log::error!("Discord client not initialized");
		instance.show_alert().await?;
		return Ok(());
	};

	if let Err(e) = client
		.emit_command(&SentCommand::SetVoiceSettings(SetVoiceSettingsArgs {
			mute: Some(false),
			..Default::default()
		}))
		.await
	{
		log::error!("SET_VOICE_SETTINGS mute=false failed: {}", e);
		instance.show_alert().await?;
		return Ok(());
	}

	let nonce = format!(
		"ptt-{}-{}",
		if talking { "down" } else { "up" },
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|d| d.as_nanos())
			.unwrap_or(0)
	);
	let payload = serde_json::json!({
		"cmd": "PUSH_TO_TALK",
		"args": { "active": talking },
		"nonce": nonce,
	});
	if let Err(e) = client.emit_string(&payload.to_string()).await {
		log::error!("PUSH_TO_TALK active={} failed: {}", talking, e);
		instance.show_alert().await?;
		return Ok(());
	}

	instance.set_state(if talking { 1 } else { 0 }).await?;
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
		push_to_talk_set(instance, true).await
	}

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		push_to_talk_set(instance, false).await
	}
}

pub struct ToggleVoiceInputModeAction;
#[async_trait]
impl Action for ToggleVoiceInputModeAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglevoiceinputmode";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let mode_lock = current_voice_mode().read().await;
		let Some(current_mode) = mode_lock.as_ref() else {
			log::error!("Voice mode not yet known");
			instance.show_alert().await?;
			return Ok(());
		};

		let is_ptt = current_mode.mode_type == "PUSH_TO_TALK";
		let new_type = if is_ptt {
			"VOICE_ACTIVITY"
		} else {
			"PUSH_TO_TALK"
		};

		let new_mode = VoiceSettingsMode {
			mode_type: new_type.to_owned(),
			..*current_mode
		};
		drop(mode_lock);

		update_voice_setting(
			instance,
			SetVoiceSettingsArgs {
				mode: Some(new_mode),
				..Default::default()
			},
			if is_ptt { 0 } else { 1 },
		)
		.await
	}
}
