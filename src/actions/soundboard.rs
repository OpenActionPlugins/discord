use crate::{
	cache::{CachedSoundboardSound, refresh_soundboard_cache, soundboard_sounds_cache},
	client::discord_client,
};

use discord_ipc_rust::models::send::commands::SentCommand;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait, visible_instances};
use serde::{Deserialize, Serialize};

pub async fn send_sounds_to_pi(instance: Option<&Instance>) {
	#[derive(Serialize)]
	struct Payload {
		sounds: Vec<CachedSoundboardSound>,
	}

	let cache = soundboard_sounds_cache().read().await;
	let payload = Payload {
		sounds: cache.clone(),
	};

	match instance {
		Some(inst) => {
			let _ = inst.send_to_property_inspector(&payload).await;
		}
		None => {
			for inst in visible_instances(PlaySoundboardSoundAction::UUID).await {
				let _ = inst.send_to_property_inspector(&payload).await;
			}
		}
	}
}

async fn send_cached_sounds_to_pi(instance: &Instance) -> OpenActionResult<()> {
	if !soundboard_sounds_cache().read().await.is_empty() {
		send_sounds_to_pi(Some(instance)).await;
		Ok(())
	} else {
		refresh_soundboard_cache(instance).await
	}
}

#[derive(Serialize, Deserialize, Default)]
pub struct PlaySoundboardSoundSettings {
	pub sound: Option<CachedSoundboardSound>,
}

pub struct PlaySoundboardSoundAction;

#[async_trait]
impl Action for PlaySoundboardSoundAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.playsoundboardsound";
	type Settings = PlaySoundboardSoundSettings;

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

		let mut client_lock = discord_client().write().await;
		let Some(client) = client_lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		let args = sound.clone().into();

		if let Err(e) = client
			.emit_command(&SentCommand::PlaySoundboardSound(args))
			.await
		{
			log::error!("Failed to play soundboard sound: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
