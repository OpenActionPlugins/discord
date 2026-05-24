use crate::client::schedule_reconnect;
use crate::current_settings;
use crate::utils::VoiceSettingsWrapper;

use discord_ipc_rust::models::receive::{
	ReceivedItem, commands::ReturnedCommand, events::ReturnedEvent,
};
use discord_ipc_rust::models::shared::voice::VoiceSettings;
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
			ReturnedEvent::VoiceSettingsUpdate(voice) => apply_voice_settings(voice).await,
			_ => {}
		},
		ReceivedItem::Command(command) => {
			if let ReturnedCommand::GetVoiceSettings(voice) = *command {
				apply_voice_settings(voice).await;
			}
		}
		ReceivedItem::SocketClosed => {
			log::warn!("Discord closed; attempting to reconnect");
			schedule_reconnect();
		}
	}
}

async fn apply_voice_settings(settings: VoiceSettings) {
	let mute = settings.mute.unwrap_or(false);
	let deaf = settings.deaf.unwrap_or(false);
	update_action_state(crate::actions::ToggleMuteAction::UUID, mute).await;
	update_action_state(crate::actions::ToggleDeafenAction::UUID, deaf).await;

	if let Some(input) = settings.input {
		*crate::actions::voice_input_settings().write().await = Some(VoiceSettingsWrapper {
			device_id: input.device_id,
			volume: input.volume,
			avaliable_devices: input.available_devices.iter().map(|d| d.into()).collect(),
			enable: !mute && !deaf,
		});
	}

	if let Some(output) = settings.output {
		*crate::actions::voice_output_settings().write().await = Some(VoiceSettingsWrapper {
			device_id: output.device_id,
			volume: output.volume,
			avaliable_devices: output.available_devices.iter().map(|d| d.into()).collect(),
			enable: !deaf,
		});
	}

	get_action_setting(crate::actions::VolumeControlAction::UUID).await; // Hacky way to refresh the state
	get_action_setting(crate::actions::SetAudioDeviceAction::UUID).await;
}

async fn update_action_state(action_uuid: ActionUuid, active: bool) {
	let state = if active { 1 } else { 0 };
	for instance in visible_instances(action_uuid).await {
		if let Err(e) = instance.set_state(state).await {
			log::error!("Failed to update state for {}: {}", action_uuid, e);
		}
	}
}

async fn get_action_setting(action_uuid: ActionUuid) {
	for instance in visible_instances(action_uuid).await {
		if let Err(e) = instance.get_settings().await {
			log::error!("Failed to get setting for {}: {}", action_uuid, e);
		}
	}
}
