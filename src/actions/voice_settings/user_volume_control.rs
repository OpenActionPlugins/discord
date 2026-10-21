use super::audio_device_utils::AudioDeviceType;

use crate::actions::audio_device_utils::user_voice_settings_map;
use crate::client::discord_client;

use base64::Engine;
use discord_ipc_rust::models::send::commands::{SentCommand, SetUserVoiceSettingsArgs};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

static ACTIVE_INSTANCES: LazyLock<
	RwLock<HashMap<String, (Arc<Instance>, UserVolumeControlSettings)>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));

#[derive(Serialize, Deserialize, Default, Clone)]
pub enum UserVolumeControlActionType {
	#[default]
	Increase,
	Decrease,
	Set,
	Mute,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct UserVolumeControlSettings {
	pub action_type: UserVolumeControlActionType,
	pub step_size: u8,
	pub set_volume: u8,
	pub user_id: Option<String>,
	pub user_nick: Option<String>,
	pub user_avatar: Option<String>,
}

impl Default for UserVolumeControlSettings {
	fn default() -> Self {
		Self {
			action_type: UserVolumeControlActionType::default(),
			step_size: 5,
			set_volume: 100,
			user_id: None,
			user_nick: None,
			user_avatar: None,
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

async fn send_users_to_pi(
	instance: &Instance,
	settings: &UserVolumeControlSettings,
) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct MinimalUser {
		pub id: String,
		pub nick: String,
	}

	#[derive(Serialize)]
	struct Payload {
		users: Vec<MinimalUser>,
		saved_nick: Option<String>,
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
		.send_to_property_inspector(Payload {
			users,
			saved_nick: settings.user_nick.clone(),
		})
		.await?;

	Ok(())
}

fn get_avatar_url(user_id: &str, avatar_hash: Option<&str>) -> String {
	match avatar_hash {
		Some(hash) => format!(
			"https://cdn.discordapp.com/avatars/{}/{}.png",
			user_id, hash
		),
		None => "https://cdn.discordapp.com/embed/avatars/0.png".to_string(),
	}
}

fn create_dimmed_svg_uri(image_data_uri: &str) -> String {
	let svg = format!(
		r#"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144">
            <filter id="dim">
                <feColorMatrix type="matrix" values="
                    0.66 0    0    0 0
                    0    0.66 0    0 0
                    0    0    0.66 0 0
                    0    0    0    1 0" />
            </filter>
            <image href="{}" width="144" height="144" filter="url(#dim)" opacity="0.4"/>
        </svg>"#,
		image_data_uri
	);
	let base64_svg = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
	format!("data:image/svg+xml;base64,{}", base64_svg)
}

async fn fetch_avatar_base64(url: &str) -> Result<String, reqwest::Error> {
	let response = reqwest::get(url).await?;
	let bytes = response.bytes().await?;
	Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

pub async fn update_action_icon(
	instance: &Instance,
	settings: &UserVolumeControlSettings,
) -> OpenActionResult<()> {
	let Some(user_id) = settings.user_id.as_ref().filter(|id| !id.is_empty()) else {
		instance.set_image(Option::<String>::None, None).await?;
		return Ok(());
	};

	let voice_map = user_voice_settings_map().read().await;
	let voice_user = voice_map.get(user_id);
	let is_in_voice = !voice_map.is_empty() && voice_user.is_some();

	let avatar_hash = voice_user
		.and_then(|u| u.avatar.as_deref())
		.or_else(|| settings.user_avatar.as_deref());

	let avatar_url = get_avatar_url(user_id, avatar_hash);

	let base64_image = match fetch_avatar_base64(&avatar_url).await {
		Ok(b64) => b64,
		Err(e) => {
			log::error!("Failed to fetch avatar from {}: {}", avatar_url, e);
			instance.show_alert().await?;
			return Ok(());
		}
	};

	let image_data_uri = format!("data:image/png;base64,{}", base64_image);

	let final_image_uri = if is_in_voice {
		image_data_uri
	} else {
		create_dimmed_svg_uri(&image_data_uri)
	};

	instance.set_image(Some(final_image_uri), None).await?;

	Ok(())
}

async fn save_user_metadata(
	instance: &Instance,
	settings: &UserVolumeControlSettings,
) -> OpenActionResult<()> {
	if let Some(user_id) = &settings.user_id {
		let voice_map = user_voice_settings_map().read().await;
		if let Some(voice_user) = voice_map.get(user_id) {
			let updated_settings = UserVolumeControlSettings {
				user_nick: Some(voice_user.nick.clone()),
				user_avatar: voice_user.avatar.clone(),
				..settings.clone()
			};
			instance.set_settings(&updated_settings).await?;
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
		save_user_metadata(instance, settings).await?;
		send_users_to_pi(instance, settings).await?;
		update_action_icon(instance, settings).await
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		save_user_metadata(instance, settings).await?;
		send_users_to_pi(instance, settings).await?;
		update_action_icon(instance, settings).await
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		ACTIVE_INSTANCES
			.write()
			.await
			.remove(&instance.instance_id.to_string());
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
