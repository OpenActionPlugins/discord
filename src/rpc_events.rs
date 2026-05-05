use crate::client::{
	fulfill_pending_command, fulfill_pending_error, schedule_reconnect,
	set_connected,
	update_selected_voice_channel_cache, update_selected_voice_channel_ids,
	update_voice_settings_cache,
};
use crate::current_settings;

use discord_ipc_rust::models::receive::{
	ReceivedItem,
	commands::ReturnedCommand,
	events::ReturnedEvent,
};
use openaction::{Action as _, ActionUuid, set_global_settings, visible_instances};

fn copy_voice_settings(
	settings: &discord_ipc_rust::models::shared::voice::VoiceSettings,
) -> Option<discord_ipc_rust::models::shared::voice::VoiceSettings> {
	serde_json::to_value(settings)
		.ok()
		.and_then(|value| serde_json::from_value(value).ok())
}

fn copy_channel(
	channel: &discord_ipc_rust::models::shared::Channel,
) -> Option<discord_ipc_rust::models::shared::Channel> {
	serde_json::to_value(channel)
		.ok()
		.and_then(|value| serde_json::from_value(value).ok())
}

pub async fn handle_rpc_event(item: ReceivedItem) {
	match item {
		ReceivedItem::Event(event) => match *event {
			ReturnedEvent::Error(error) => {
				let consumed = fulfill_pending_error(error.code, error.message.clone()).await;
				log::error!(
					"Discord RPC error: code {:?}, message {:?}",
					error.code,
					error.message
				);

				if !consumed && error.code == 4006 {
					let mut current = current_settings().write().await;
					current.access_token.clear();
					if let Err(e) = set_global_settings(&*current).await {
						log::error!("Failed to clear access token in settings: {}", e);
					}
					schedule_reconnect();
				}
			}
			ReturnedEvent::VoiceSettingsUpdate(voice) => {
				if let Some(copy) = copy_voice_settings(&voice) {
					update_voice_settings_cache(copy).await;
				}
				apply_voice_state(voice.mute, voice.deaf).await;
			}
			ReturnedEvent::VoiceChannelSelect(selection) => {
				let channel_id = if selection.channel_id.is_empty() {
					None
				} else {
					Some(selection.channel_id)
				};
				let guild_id = if selection.guild_id.is_empty() {
					None
				} else {
					Some(selection.guild_id)
				};
				update_selected_voice_channel_ids(channel_id, guild_id).await;
				crate::actions::sync_join_voice_channel_states().await;
			}
			_ => {}
		},
		ReceivedItem::Command(command) => {
			match &*command {
				ReturnedCommand::GetVoiceSettings(voice)
				| ReturnedCommand::SetVoiceSettings(voice) => {
					if let Some(copy) = copy_voice_settings(voice) {
						update_voice_settings_cache(copy).await;
					}
					apply_voice_state(voice.mute, voice.deaf).await;
				}
				ReturnedCommand::GetSelectedVoiceChannel(channel)
				| ReturnedCommand::SelectVoiceChannel(channel) => {
					let copy = channel.as_ref().and_then(copy_channel);
					update_selected_voice_channel_cache(copy.as_ref()).await;
					crate::actions::sync_join_voice_channel_states().await;
				}
				ReturnedCommand::SelectTextChannel(_)
				| ReturnedCommand::Subscribe { .. }
				| ReturnedCommand::Unsubscribe { .. }
				| ReturnedCommand::Authorize { .. }
				| ReturnedCommand::Authenticate(_)
				| ReturnedCommand::GetGuild(_)
				| ReturnedCommand::GetGuilds(_)
				| ReturnedCommand::GetChannel(_)
				| ReturnedCommand::GetChannels(_)
				| ReturnedCommand::SetUserVoiceSettings(_)
				| ReturnedCommand::SetCertifiedDevices
				| ReturnedCommand::SetActivity
				| ReturnedCommand::SendActivityJoinInvite
				| ReturnedCommand::CloseActivityRequest => {}
			}

			let _ = fulfill_pending_command(&command).await;
		}
		ReceivedItem::SocketClosed => {
			log::warn!("Discord closed; attempting to reconnect");
			set_connected(false).await;
			schedule_reconnect();
		}
	}
}

async fn apply_voice_state(mute: Option<bool>, deaf: Option<bool>) {
	let mute = mute.unwrap_or(false) || deaf.unwrap_or(false);
	let deaf = deaf.unwrap_or(false);
	update_action_state(crate::actions::ToggleMuteAction::UUID, mute).await;
	update_action_state(crate::actions::ToggleDeafenAction::UUID, deaf).await;
}

async fn update_action_state(action_uuid: ActionUuid, active: bool) {
	let state = if active { 1 } else { 0 };
	for instance in visible_instances(action_uuid).await {
		if let Err(e) = instance.set_state(state).await {
			log::error!("Failed to update state for {}: {}", action_uuid, e);
		}
	}
}
