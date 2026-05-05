use crate::actions::PluginActionSettings;
use crate::client::{request_voice_settings, set_voice_settings, set_voice_settings_json, update_error};

use std::sync::atomic::Ordering::Relaxed;

use discord_ipc_rust::models::send::commands::SetVoiceSettingsArgs;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde_json::json;

async fn fail(instance: &Instance, message: &str) -> OpenActionResult<()> {
	log::error!("{}", message);
	update_error(message).await;
	instance.show_alert().await?;
	Ok(())
}

async fn update_voice_setting(
	instance: &Instance,
	args: SetVoiceSettingsArgs,
	next_state: usize,
) -> OpenActionResult<()> {
	match set_voice_settings(args).await {
		Ok(_) => instance.set_state(next_state as u16).await,
		Err(error) => fail(instance, &error).await,
	}
}

pub struct ToggleMuteAction;
#[async_trait]
impl Action for ToggleMuteAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglemute";
	type Settings = PluginActionSettings;

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
	type Settings = PluginActionSettings;

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
	type Settings = PluginActionSettings;

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
	type Settings = PluginActionSettings;

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

pub struct ToggleVoiceModeAction;
#[async_trait]
impl Action for ToggleVoiceModeAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglevoicemode";
	type Settings = PluginActionSettings;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let voice = match request_voice_settings().await {
			Ok(voice) => voice,
			Err(error) => return fail(instance, &error).await,
		};

		let Some(mode) = voice.mode else {
			return fail(
				instance,
				"Discord did not return voice mode details, so Toggle Voice Mode is unavailable",
			)
			.await;
		};

		let next_mode_type = match mode.mode_type.as_str() {
			"VOICE_ACTIVITY" => "PUSH_TO_TALK",
			"PUSH_TO_TALK" => "VOICE_ACTIVITY",
			other => {
				return fail(
					instance,
					&format!("Unsupported Discord voice mode \"{other}\""),
				)
				.await;
			}
		};

		match set_voice_settings_json(json!({
			"mode": {
				"type": next_mode_type,
				"auto_threshold": mode.auto_threshold,
				"threshold": mode.threshold,
				"delay": mode.delay
			}
		}))
		.await
		{
			Ok(updated) => {
				let state = updated
					.mode
					.as_ref()
					.map(|value| value.mode_type.as_str() == "PUSH_TO_TALK")
					.unwrap_or(next_mode_type == "PUSH_TO_TALK");
				instance.set_state(if state { 1 } else { 0 }).await
			}
			Err(error) => fail(instance, &error).await,
		}
	}
}
