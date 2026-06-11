use crate::client::discord_client;

use std::sync::OnceLock;

use discord_ipc_rust::models::{
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
}

impl From<CachedSoundboardSound> for PlaySoundboardSoundArgs {
	fn from(sound: CachedSoundboardSound) -> Self {
		Self {
			guild_id: sound.guild_id,
			sound_id: sound.sound_id,
		}
	}
}

pub fn guild_cache() -> &'static RwLock<Vec<CachedGuild>> {
	static CACHE: OnceLock<RwLock<Vec<CachedGuild>>> = OnceLock::new();
	CACHE.get_or_init(|| RwLock::new(Vec::new()))
}

pub fn soundboard_sounds_cache() -> &'static RwLock<Vec<CachedSoundboardSound>> {
	static CACHE: OnceLock<RwLock<Vec<CachedSoundboardSound>>> = OnceLock::new();
	CACHE.get_or_init(|| RwLock::new(Vec::new()))
}

pub async fn update_guild_cache(guilds: &[Guild]) {
	let mut cached: Vec<CachedGuild> = guilds
		.iter()
		.map(|g| CachedGuild {
			id: g.id.clone(),
			name: g.name.clone(),
		})
		.collect();
	cached.sort_by_key(|x| x.name.to_lowercase());
	*guild_cache().write().await = cached;
}

pub async fn refresh_guild_cache(instance: &Instance) -> OpenActionResult<()> {
	let mut client_lock = discord_client().write().await;
	if let Some(client) = client_lock.as_mut()
		&& let Err(e) = client.emit_command(&SentCommand::GetGuilds).await
	{
		log::error!("Failed to request guilds: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}

pub async fn update_soundboard_cache(sounds: &[SoundboardSound]) {
	let mut cached: Vec<CachedSoundboardSound> = sounds
		.iter()
		.filter(|s| !s.available)
		.map(|s| CachedSoundboardSound {
			name: s.name.clone(),
			guild_id: s.guild_id.clone(),
			sound_id: s.sound_id.clone(),
		})
		.collect();
	cached.sort_by_key(|x| x.name.to_lowercase());
	*soundboard_sounds_cache().write().await = cached;
}

pub async fn refresh_soundboard_cache(instance: &Instance) -> OpenActionResult<()> {
	let mut client_lock = discord_client().write().await;
	if let Some(client) = client_lock.as_mut()
		&& let Err(e) = client.emit_command(&SentCommand::GetSoundboardSounds).await
	{
		log::error!("Failed to request soundboard sounds: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}
