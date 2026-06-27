use crate::actions::audio_device_utils::{
	AudioDeviceType, AudioDeviceWrapper, user_voice_settings_map,
};
use crate::client::{current_user_id, current_voice_channel, schedule_reconnect};
use crate::current_settings;

use discord_ipc_rust::models::receive::events::NotificationCreateData;
use discord_ipc_rust::models::receive::{
	ReceivedItem, commands::ReturnedCommand, events::ReturnedEvent,
};
use openaction::{Action as _, ActionUuid, set_global_settings, visible_instances};

// Central handler for Discord RPC events and command responses we subscribe to (e.g., voice settings).
pub async fn handle_rpc_event(item: ReceivedItem) {
	match item {
		ReceivedItem::Event(event) => match *event {
			ReturnedEvent::Error(error) => {
				log::error!(
					"Discord RPC error: code {:?}, message {:?}",
					error.code,
					error.message
				);
				if error.code == 4006 {
					let mut current = current_settings().write().await;
					current.access_token.clear();
					if let Err(e) = set_global_settings(&*current).await {
						log::error!("Failed to clear access token in settings: {}", e);
					}
					schedule_reconnect();
				}
			}
			ReturnedEvent::VoiceSettingsUpdate(voice) => apply_voice_state(voice).await,
			ReturnedEvent::VoiceStateCreate(state) | ReturnedEvent::VoiceStateUpdate(state) => {
				let Some(user) = &state.user else {
					return;
				};

				let current_user_id = current_user_id().read().await;

				if current_user_id.as_ref() != Some(&user.id) {
					user_voice_settings_map()
						.write()
						.await
						.insert(user.id.clone(), state.into());

					for instance in
						visible_instances(crate::actions::UserVolumeControlAction::UUID).await
					{
						let _ = instance.get_settings().await;
					}
				}
			}
			ReturnedEvent::VoiceStateDelete(state) => {
				if let Some(user) = &state.user {
					user_voice_settings_map().write().await.remove(&user.id);

					for instance in
						visible_instances(crate::actions::UserVolumeControlAction::UUID).await
					{
						let _ = instance.get_settings().await;
					}
				}
			}
			ReturnedEvent::VideoStateUpdate(state) => {
				update_action_state(crate::actions::ToggleVideoAction::UUID, state.active).await;
			}
			ReturnedEvent::ScreenshareStateUpdate(state) => {
				update_action_state(crate::actions::ToggleScreenshareAction::UUID, state.active)
					.await;
			}
			ReturnedEvent::VoiceChannelSelect(data) => {
				handle_select_voice_channel(data.channel_id).await;
			}
			ReturnedEvent::NotificationCreate(notification) => {
				handle_notification(notification).await;
			}
			_ => {}
		},
		ReceivedItem::Command(command) => match *command {
			ReturnedCommand::GetVoiceSettings(voice) => {
				apply_voice_state(voice).await;
			}
			ReturnedCommand::GetGuilds { guilds } => {
				crate::cache::update_guild_cache(&guilds).await;
				crate::actions::channel::send_guilds_to_pi(None).await;
			}
			ReturnedCommand::GetChannels { channels } => {
				crate::actions::channel::send_channels_to_pi(&channels).await;
			}
			ReturnedCommand::GetSelectedVoiceChannel(channel) => {
				let channel_id = channel.map(|c| c.id);
				handle_select_voice_channel(channel_id).await;
			}
			ReturnedCommand::GetSoundboardSounds(sounds) => {
				crate::cache::update_soundboard_cache(&sounds).await;
				crate::actions::soundboard::send_sounds_to_pi(None).await;
			}
			_ => {}
		},
		ReceivedItem::SocketClosed => {
			log::warn!("Discord closed; attempting to reconnect");
			crate::cache::guild_cache().write().await.clear();
			user_voice_settings_map().write().await.clear();
			schedule_reconnect();
		}
	}
}

async fn handle_select_voice_channel(channel_id: Option<String>) {
	let old_channel = current_voice_channel().read().await.clone();

	if old_channel == channel_id {
		return;
	}

	if let Some(old_channel) = old_channel {
		crate::client::update_voice_state_subscription(old_channel, false).await;
		user_voice_settings_map().write().await.clear();
	}

	*current_voice_channel().write().await = channel_id.clone();

	if let Some(new_channel) = channel_id {
		crate::client::update_voice_state_subscription(new_channel, true).await;
	}

	for instance in visible_instances(crate::actions::VoiceChannelAction::UUID).await {
		let _ = instance.get_settings().await;
	}
}

async fn handle_notification(notification: NotificationCreateData) {
	crate::cache::add_notification_to_cache(notification).await;
	for instance in visible_instances(crate::actions::NotificationsAction::UUID).await {
		let _ = crate::actions::notifications::update_title(&instance).await;
	}
}

async fn apply_voice_state(settings: discord_ipc_rust::models::shared::voice::VoiceSettings) {
	let mute = settings.mute.unwrap_or(false);
	let deaf = settings.deaf.unwrap_or(false);
	update_action_state(crate::actions::ToggleMuteAction::UUID, mute || deaf).await;
	update_action_state(crate::actions::ToggleDeafenAction::UUID, deaf).await;

	if let Some(mode) = &settings.mode {
		let is_ptt = mode.mode_type == "PUSH_TO_TALK";
		update_action_state(crate::actions::ToggleVoiceInputModeAction::UUID, is_ptt).await;
		*crate::actions::current_voice_mode().write().await =
			Some(discord_ipc_rust::models::shared::voice::VoiceSettingsMode {
				mode_type: mode.mode_type.clone(),
				..*mode
			});
	}

	if let Some(input) = settings.input {
		*crate::actions::audio_device_utils::audio_input_settings()
			.write()
			.await = Some(AudioDeviceWrapper {
			device_type: AudioDeviceType::Input,
			device_id: input.device_id,
			volume: input.volume,
			available_devices: input.available_devices,
			enable: !mute && !deaf,
		});
	}

	if let Some(output) = settings.output {
		*crate::actions::audio_device_utils::audio_output_settings()
			.write()
			.await = Some(AudioDeviceWrapper {
			device_type: AudioDeviceType::Output,
			device_id: output.device_id,
			volume: output.volume,
			available_devices: output.available_devices,
			enable: !deaf,
		});
	}

	for instance in visible_instances(crate::actions::VolumeControlAction::UUID).await {
		let _ = instance.get_settings().await;
	}

	for instance in visible_instances(crate::actions::SetAudioDeviceAction::UUID).await {
		let _ = crate::actions::voice_settings::set_audio_device::send_available_devices_to_pi(
			&instance,
		)
		.await;
	}
}

async fn update_action_state(action_uuid: ActionUuid, active: bool) {
	let state = if active { 1 } else { 0 };
	for instance in visible_instances(action_uuid).await {
		if let Err(e) = instance.set_state(state).await {
			log::error!("Failed to update state for {}: {}", action_uuid, e);
		}
	}
}
