use crate::client::get_discord_client;

use std::collections::VecDeque;
use std::sync::LazyLock;

use discord_ipc_rust::models::{
	receive::events::NotificationCreateData,
	send::commands::{PlaySoundboardSoundArgs, SentCommand},
	shared::{Guild, voice::SoundboardSound},
};
use openaction::{Instance, OpenActionResult};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Serialize, Clone)]
pub struct CachedGuild {
	id: String,
	name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedSoundboardSound {
	pub name: String,
	pub guild_id: String,
	pub sound_id: String,
	pub emoji_id: Option<String>,
	pub emoji_name: Option<String>,
}

impl From<CachedSoundboardSound> for PlaySoundboardSoundArgs {
	fn from(sound: CachedSoundboardSound) -> Self {
		Self {
			guild_id: sound.guild_id,
			sound_id: sound.sound_id,
		}
	}
}

#[derive(Serialize, Clone)]
pub struct CachedNotification {
	pub channel_id: String,
}

pub static GUILD_CACHE: LazyLock<RwLock<Vec<CachedGuild>>> =
	LazyLock::new(|| RwLock::new(Vec::new()));

pub static SOUNDBOARD_SOUNDS_CACHE: LazyLock<RwLock<Vec<CachedSoundboardSound>>> =
	LazyLock::new(|| RwLock::new(Vec::new()));

pub static NOTIFICATION_CACHE: LazyLock<RwLock<VecDeque<CachedNotification>>> =
	LazyLock::new(|| RwLock::new(VecDeque::new()));

pub async fn update_guild_cache(guilds: &[Guild]) {
	let mut cached: Vec<CachedGuild> = guilds
		.iter()
		.map(|g| CachedGuild {
			id: g.id.clone(),
			name: g.name.clone(),
		})
		.collect();
	cached.sort_by_key(|x| x.name.to_lowercase());
	*GUILD_CACHE.write().await = cached;
}

pub async fn refresh_guild_cache(instance: &Instance) -> OpenActionResult<()> {
	let result = {
		let Some(mut client) = get_discord_client(instance).await? else {
			return Ok(());
		};

		client.emit_command(&SentCommand::GetGuilds).await
	};

	if let Err(e) = result {
		log::error!("Failed to request guilds: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}

pub async fn update_soundboard_cache(sounds: &[SoundboardSound]) {
	let mut cached: Vec<CachedSoundboardSound> = sounds
		.iter()
		.map(|s| CachedSoundboardSound {
			name: s.name.clone(),
			guild_id: s.guild_id.clone(),
			sound_id: s.sound_id.clone(),
			emoji_id: s.emoji_id.clone(),
			emoji_name: s.emoji_name.clone(),
		})
		.collect();
	cached.sort_by_key(|x| x.name.to_lowercase());
	*SOUNDBOARD_SOUNDS_CACHE.write().await = cached;
}

pub async fn refresh_soundboard_cache(instance: &Instance) -> OpenActionResult<()> {
	let result = {
		let Some(mut client) = get_discord_client(instance).await? else {
			return Ok(());
		};

		client.emit_command(&SentCommand::GetSoundboardSounds).await
	};

	if let Err(e) = result {
		log::error!("Failed to request soundboard sounds: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}

pub async fn add_notification_to_cache(notification: NotificationCreateData) {
	let mut cache_lock = NOTIFICATION_CACHE.write().await;
	cache_lock.push_back(CachedNotification {
		channel_id: notification.channel_id,
	});
}
