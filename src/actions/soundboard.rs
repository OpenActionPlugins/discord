use crate::cache::{CachedSoundboardSound, refresh_soundboard_cache, soundboard_sounds_cache};
use crate::client::discord_client;
use crate::image_utils;

use discord_ipc_rust::models::send::commands::SentCommand;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait, visible_instances};
use serde::{Deserialize, Serialize};

const SOUND_ICON_SIZE: u32 = 96;
const TWEMOJI_VERSION: &str = "v14.0.2";

// Converts an emoji string into Twemoji's asset naming convention: lowercase hex codepoints
// joined by '-', dropping the invisible VARIATION SELECTOR-16 character.
fn twemoji_codepoint(emoji: &str) -> String {
	emoji
		.chars()
		.map(|c| c as u32)
		.filter(|&codepoint| codepoint != 0xFE0F)
		.map(|codepoint| format!("{codepoint:x}"))
		.collect::<Vec<_>>()
		.join("-")
}

// Resolves the CDN URL for a sound's emoji: Discord's own CDN for a custom server emoji, or
// the Twemoji project's CDN (the same artwork Discord itself uses on the web) for a standard
// Unicode one. Returns `None` if the sound has no emoji to render at all.
fn emoji_icon_url(sound: &CachedSoundboardSound) -> Option<String> {
	if let Some(emoji_id) = &sound.emoji_id {
		return Some(format!(
			"https://cdn.discordapp.com/emojis/{emoji_id}.png?size=96"
		));
	}

	let codepoint = twemoji_codepoint(sound.emoji_name.as_deref()?);
	if codepoint.is_empty() {
		return None;
	}

	Some(format!(
		"https://cdn.jsdelivr.net/gh/twitter/twemoji@{TWEMOJI_VERSION}/assets/72x72/{codepoint}.png"
	))
}

// Resolves a sound's icon as a data URI. `Ok(None)` means the sound genuinely has no emoji to
// show (reset to the default icon); `Err(())` means fetching/decoding it failed transiently
// (leave whatever icon is already displayed alone, rather than flickering back to default).
async fn sound_icon_data_uri(sound: &CachedSoundboardSound) -> Result<Option<String>, ()> {
	let Some(icon_url) = emoji_icon_url(sound) else {
		return Ok(None);
	};

	let Some(bytes) = image_utils::fetch_bytes(&icon_url).await else {
		return Err(());
	};

	match image_utils::resized_png(&bytes, SOUND_ICON_SIZE) {
		Some(png_bytes) => Ok(Some(image_utils::to_data_uri(&png_bytes))),
		None => {
			log::error!(
				"Failed to render icon for sound '{}' from {}",
				sound.name,
				icon_url
			);
			Err(())
		}
	}
}

pub async fn send_sounds_to_pi(instance: Option<&Instance>) {
	#[derive(Serialize)]
	struct Payload {
		sounds: Vec<CachedSoundboardSound>,
	}

	let payload = Payload {
		sounds: soundboard_sounds_cache().read().await.clone(),
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

async fn set_button_display(instance: &Instance, sound: Option<&CachedSoundboardSound>) {
	let title = sound.map(|s| s.name.chars().take(8).collect::<String>());
	let _ = instance.set_title(title, None).await;

	let icon = match sound {
		None => None,
		Some(sound) => match sound_icon_data_uri(sound).await {
			Ok(icon) => icon,
			// Transient failure: leave whatever icon is already shown alone.
			Err(()) => return,
		},
	};

	if let Err(e) = instance.set_image(icon, None).await {
		log::error!("Failed to set image for soundboard action: {}", e);
	}
}

async fn send_cached_sounds_to_pi(instance: &Instance) -> OpenActionResult<()> {
	if !soundboard_sounds_cache().read().await.is_empty() {
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
		set_button_display(instance, settings.sound.as_ref()).await;
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		set_button_display(instance, settings.sound.as_ref()).await;
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

		let mut client_lock = discord_client().write().await;
		let Some(client) = client_lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		if let Err(e) = client
			.emit_command(&SentCommand::PlaySoundboardSound(sound.clone().into()))
			.await
		{
			log::error!("Failed to play soundboard sound: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
