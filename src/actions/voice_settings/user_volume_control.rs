use super::audio_device_utils::{AudioDeviceType, avatar_data_uri, avatar_url};

use crate::actions::audio_device_utils::user_voice_settings_map;
use crate::client::discord_client;

use discord_ipc_rust::models::send::commands::{SentCommand, SetUserVoiceSettingsArgs};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use serde_json::json;

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
		log::error!("Failed to update user voice settings: {}", e);
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
		pub avatar: String,
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
			avatar: avatar_url(user_id, settings.avatar.as_deref()),
		})
		.collect();

	instance
		.send_to_property_inspector(Payload { users })
		.await?;

	Ok(())
}

// Renders the target user's avatar, name and current volume onto the physical key/dial:
// a title overlaid on the avatar image for Keypad instances, or the native "$B1" indicator
// layout (icon + title + volume bar) for Encoder (Stream Deck+ dial) instances. The layout
// itself is declared as this action's default in the manifest and is never re-sent from here:
// calling SetFeedbackLayout on every update would reset the dial's contents to blank before
// this function repopulates them, causing a visible flicker back to placeholder text/icons.
async fn update_display(instance: &Instance, user_id: &str) -> OpenActionResult<()> {
	let Some(user_settings) = user_voice_settings_map().read().await.get(user_id).cloned() else {
		return Ok(());
	};

	let percent = AudioDeviceType::Output
		.to_linear(user_settings.volume)
		.round() as i64;
	let value_text = if user_settings.mute {
		"Muted".to_owned()
	} else {
		format!("{percent}%")
	};
	// If the avatar couldn't be fetched (e.g. a transient network error), leave whatever image
	// is already displayed alone rather than clearing it back to the action's default icon.
	let icon = avatar_data_uri(
		user_id,
		user_settings.avatar.as_deref(),
		user_settings.speaking,
	)
	.await;

	if instance.controller == "Encoder" {
		let bar_fill_color = if percent > 100 { "red" } else { "white" };

		let mut feedback = json!({
			"title": user_settings.nick,
			"value": value_text,
			"indicator": { "value": percent.clamp(0, 100), "bar_fill_c": bar_fill_color },
		});
		if let Some(icon) = icon {
			feedback["icon"] = json!(icon);
		}

		if let Err(e) = instance.set_feedback(&feedback).await {
			log::error!("Failed to set feedback for user volume control: {}", e);
		}
	} else {
		if let Some(icon) = icon
			&& let Err(e) = instance.set_image(Some(icon), None).await
		{
			log::error!("Failed to set image for user volume control: {}", e);
		}

		if let Err(e) = instance
			.set_title(Some(format!("{}\n{value_text}", user_settings.nick)), None)
			.await
		{
			log::error!("Failed to set title for user volume control: {}", e);
		}
	}

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
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_users_to_pi(instance).await?;
		if let Some(user_id) = settings.user_id.as_ref() {
			update_display(instance, user_id).await?;
		}
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_users_to_pi(instance).await?;
		if let Some(user_id) = settings.user_id.as_ref() {
			update_display(instance, user_id).await?;
		}
		Ok(())
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let Some(user_id) = settings.user_id.as_ref() else {
			log::error!("Failed to update user voice settings: no user ID provided");
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
			log::error!("Failed to adjust user volume: no user ID provided");
			instance.show_alert().await?;
			Ok(())
		}
	}
}
