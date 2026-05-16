use crate::actions::VolumeChangeSettings;
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
			ReturnedEvent::VoiceSettingsUpdate(voice) => apply_voice_state(voice).await,
			ReturnedEvent::VideoStateUpdate(state) => {
				update_action_state(crate::actions::ToggleVideoAction::UUID, state.active).await;
			}
			ReturnedEvent::ScreenshareStateUpdate(state) => {
				update_action_state(crate::actions::ToggleScreenshareAction::UUID, state.active)
					.await;
			}
			_ => {}
		},
		ReceivedItem::Command(command) => {
			if let ReturnedCommand::GetVoiceSettings(voice) = *command {
				apply_voice_state(voice).await;
			}
		}
		ReceivedItem::SocketClosed => {
			log::warn!("Discord closed; attempting to reconnect");
			schedule_reconnect();
		}
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

	update_action_state(crate::actions::InputVolumeChangeAction::UUID, mute).await;
	update_action_state(crate::actions::OutputVolumeChangeAction::UUID, deaf).await;

	update_action_settings(
		crate::actions::InputVolumeChangeAction::UUID,
		VolumeChangeSettings {
			pressing: false,
			toggle: mute,
			data: settings.input.map(|i| i.into()),
		},
	)
	.await;

	update_action_settings(
		crate::actions::OutputVolumeChangeAction::UUID,
		VolumeChangeSettings {
			pressing: false,
			toggle: deaf,
			data: settings.output.map(|o| o.into()),
		},
	)
	.await;
}

async fn update_action_state(action_uuid: ActionUuid, active: bool) {
	let state = if active { 1 } else { 0 };
	for instance in visible_instances(action_uuid).await {
		if let Err(e) = instance.set_state(state).await {
			log::error!("Failed to update state for {}: {}", action_uuid, e);
		}
	}
}

async fn update_action_settings(action_uuid: ActionUuid, settings: impl serde::Serialize) {
	for instance in visible_instances(action_uuid).await {
		if let Err(e) = instance.set_settings(&settings).await {
			log::error!("Failed to update settings for {}: {}", action_uuid, e);
		}
	}
}
