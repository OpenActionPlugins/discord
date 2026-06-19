use crate::CURRENT_SETTINGS;
use crate::audio_device_utils::{AudioDeviceType, AudioDeviceWrapper};
use crate::client::{
	AUDIO_INPUT_TYPE, AUDIO_OUTPUT_TYPE, CURRENT_USER_ID, CURRENT_VOICE_CHANNEL,
	CURRENT_VOICE_MODE, USER_VOICE_SETTINGS_MAP, schedule_reconnect,
};

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
					let mut current = CURRENT_SETTINGS.write().await;
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

				let current_user_id = CURRENT_USER_ID.read().await;

				if current_user_id.as_ref() != Some(&user.id) {
					USER_VOICE_SETTINGS_MAP
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
					USER_VOICE_SETTINGS_MAP.write().await.remove(&user.id);

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
			_ => {}
		},
		ReceivedItem::SocketClosed => {
			log::warn!("Discord closed; attempting to reconnect");
			crate::cache::guild_cache().write().await.clear();
			USER_VOICE_SETTINGS_MAP.write().await.clear();
			schedule_reconnect();
		}
	}
}

async fn handle_select_voice_channel(channel_id: Option<String>) {
	let old_channel = CURRENT_VOICE_CHANNEL.read().await.clone();

	if old_channel == channel_id {
		return;
	}

	if let Some(old_channel) = old_channel {
		crate::client::update_voice_state_subscription(old_channel, false).await;
		USER_VOICE_SETTINGS_MAP.write().await.clear();
	}

	*CURRENT_VOICE_CHANNEL.write().await = channel_id.clone();

	if let Some(new_channel) = channel_id {
		crate::client::update_voice_state_subscription(new_channel, true).await;
	}

	for instance in visible_instances(crate::actions::VoiceChannelAction::UUID).await {
		let _ = instance.get_settings().await;
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
		*CURRENT_VOICE_MODE.write().await =
			Some(discord_ipc_rust::models::shared::voice::VoiceSettingsMode {
				mode_type: mode.mode_type.clone(),
				..*mode
			});
	}

	if let Some(input) = settings.input {
		*AUDIO_INPUT_TYPE.write().await = Some(AudioDeviceWrapper {
			device_type: AudioDeviceType::Input,
			device_id: input.device_id,
			volume: input.volume,
			available_devices: input.available_devices,
		});
	}

	if let Some(output) = settings.output {
		*AUDIO_OUTPUT_TYPE.write().await = Some(AudioDeviceWrapper {
			device_type: AudioDeviceType::Output,
			device_id: output.device_id,
			volume: output.volume,
			available_devices: output.available_devices,
		});
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
