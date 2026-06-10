use crate::actions::audio_device_utils::user_voice_settings_map;
use crate::client::discord_client;

use super::audio_device_utils::AudioDeviceType;

use discord_ipc_rust::models::send::commands::{SentCommand, SetUserVoiceSettingsArgs};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub enum UserVolumeControlActionType {
	#[default]
	Increase,
	Decrease,
	Set,
	Mute,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct UserVolumeControlSettings {
	pub action_type: UserVolumeControlActionType,
	pub step_size: u8,
	pub set_volume: u8,
	pub user_id: Option<String>,
}

impl Default for UserVolumeControlSettings {
	fn default() -> Self {
		Self {
			action_type: UserVolumeControlActionType::default(),
			step_size: 5,
			set_volume: 100,
			user_id: None,
		}
	}
}

async fn update_user_voice_settings(
	instance: &Instance,
	args: SetUserVoiceSettingsArgs,
) -> OpenActionResult<()> {
	let mut client_lock = discord_client().write().await;
	let Some(client) = client_lock.as_mut() else {
		log::error!("Discord client not initialized");
		instance.show_alert().await?;
		return Ok(());
	};

	if let Err(e) = client
		.emit_command(&SentCommand::SetUserVoiceSettings(args))
		.await
	{
		log::error!("Failed to update voice state: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}

async fn adjust_user_volume(
	instance: &Instance,
	user_id: String,
	value: f32,
	set: bool,
) -> OpenActionResult<()> {
	let device_type = AudioDeviceType::Output;

	let current_volume = match user_voice_settings_map().read().await.get(&user_id) {
		Some(settings) => settings.volume,
		None => {
			log::error!(
				"Failed to adjust volume for user '{}': user not found in voice settings map",
				user_id
			);
			instance.show_alert().await?;
			return Ok(());
		}
	};

	let new_volume = if set {
		value.clamp(0.0, device_type.max_volume())
	} else {
		(device_type.to_linear(current_volume) + value).clamp(0.0, device_type.max_volume())
	};

	if new_volume == current_volume {
		return Ok(());
	}

	update_user_voice_settings(
		instance,
		SetUserVoiceSettingsArgs {
			user_id,
			pan: None,
			volume: Some(device_type.to_discord(new_volume)),
			mute: None,
		},
	)
	.await
}

async fn send_users_to_pi(instance: &Instance) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct MinimalUser {
		pub id: String,
		pub nick: String,
	}

	#[derive(Serialize)]
	struct Payload {
		users: Vec<MinimalUser>,
	}

	let users = user_voice_settings_map()
		.read()
		.await
		.iter()
		.map(|(user_id, settings)| MinimalUser {
			id: user_id.clone(),
			nick: settings.nick.clone(),
		})
		.collect();

	instance
		.send_to_property_inspector(Payload { users })
		.await?;

	Ok(())
}
pub struct UserVolumeControlAction;
#[async_trait]
impl Action for UserVolumeControlAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.uservolumecontrol";
	type Settings = UserVolumeControlSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_users_to_pi(instance).await
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_users_to_pi(instance).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let Some(user_id) = settings.user_id.as_ref() else {
			log::error!("User ID not specified in settings");
			instance.show_alert().await?;
			return Ok(());
		};

		if matches!(settings.action_type, UserVolumeControlActionType::Mute) {
			let new_mute_state = match user_voice_settings_map().read().await.get(user_id) {
				Some(settings) => !settings.mute,
				None => {
					log::error!(
						"Failed to toggle mute for user '{}': user not found in voice settings map",
						user_id
					);
					instance.show_alert().await?;
					return Ok(());
				}
			};

			return update_user_voice_settings(
				instance,
				SetUserVoiceSettingsArgs {
					user_id: user_id.clone(),
					pan: None,
					volume: None,
					mute: Some(new_mute_state),
				},
			)
			.await;
		}

		let value = match settings.action_type {
			UserVolumeControlActionType::Increase => settings.step_size as f32,
			UserVolumeControlActionType::Decrease => -(settings.step_size as f32),
			UserVolumeControlActionType::Set => settings.set_volume as f32,
			UserVolumeControlActionType::Mute => unreachable!(),
		};

		adjust_user_volume(
			instance,
			user_id.clone(),
			value,
			matches!(settings.action_type, UserVolumeControlActionType::Set),
		)
		.await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let delta = (settings.step_size as f32) * ticks as f32;

		if let Some(user_id) = &settings.user_id {
			adjust_user_volume(instance, user_id.clone(), delta, false).await
		} else {
			log::error!("User ID not specified in settings");
			instance.show_alert().await?;
			Ok(())
		}
	}
}
