use crate::cache::{CachedSoundboardSound, SOUNDBOARD_SOUNDS_CACHE, refresh_soundboard_cache};
use crate::client::get_discord_client;

use discord_ipc_rust::models::send::commands::SentCommand;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait, visible_instances};
use serde::{Deserialize, Serialize};

pub async fn send_sounds_to_pi(instance: Option<&Instance>) {
	#[derive(Serialize)]
	struct Payload {
		sounds: Vec<CachedSoundboardSound>,
	}

	let payload = Payload {
		sounds: SOUNDBOARD_SOUNDS_CACHE.read().await.clone(),
	};

	match instance {
		Some(inst) => {
			let _ = inst.send_to_property_inspector(&payload).await;
		}
		None => {
			for inst in visible_instances(SoundboardAction::UUID).await {
				let _ = inst.send_to_property_inspector(&payload).await;
			}
		}
	}
}

async fn set_button_title(instance: &Instance, sound: Option<&CachedSoundboardSound>) {
	let title = sound.map(|s| s.name.chars().take(8).collect::<String>());
	let _ = instance.set_title(title, None).await;
}

async fn send_cached_sounds_to_pi(instance: &Instance) -> OpenActionResult<()> {
	if !SOUNDBOARD_SOUNDS_CACHE.read().await.is_empty() {
		send_sounds_to_pi(Some(instance)).await;
		crate::actions::channel::send_cached_guilds_to_pi(instance).await?;
		Ok(())
	} else {
		refresh_soundboard_cache(instance).await
	}
}

#[derive(Serialize, Deserialize, Default)]
pub struct SoundboardSettings {
	pub sound: Option<CachedSoundboardSound>,
}

pub struct SoundboardAction;

#[async_trait]
impl Action for SoundboardAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.soundboard";
	type Settings = SoundboardSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		set_button_title(instance, settings.sound.as_ref()).await;
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		set_button_title(instance, settings.sound.as_ref()).await;
		Ok(())
	}

	async fn property_inspector_did_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_cached_sounds_to_pi(instance).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let Some(sound) = &settings.sound else {
			instance.show_alert().await?;
			return Ok(());
		};

		let result = {
			let Some(mut client) = get_discord_client(instance).await? else {
				return Ok(());
			};
			client
				.emit_command(&SentCommand::PlaySoundboardSound(sound.clone().into()))
				.await
		};

		if let Err(e) = result {
			log::error!("Failed to play soundboard sound: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
