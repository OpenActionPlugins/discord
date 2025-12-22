use crate::client::schedule_reconnect;
use crate::current_settings;

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
			ReturnedEvent::VoiceSettingsUpdate(voice) => {
				apply_voice_state(voice.mute, voice.deaf).await
			}
			_ => {}
		},
		ReceivedItem::Command(command) => {
			if let ReturnedCommand::GetVoiceSettings(voice) = *command {
				apply_voice_state(voice.mute, voice.deaf).await;
			}
		}
		ReceivedItem::SocketClosed => {
			log::warn!("Discord closed; attempting to reconnect");
			schedule_reconnect();
		}
	}
}

async fn apply_voice_state(mute: Option<bool>, deaf: Option<bool>) {
	let mute = mute.unwrap_or(false);
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
