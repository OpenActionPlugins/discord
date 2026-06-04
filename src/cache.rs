use crate::client::discord_client;

use std::sync::OnceLock;

use discord_ipc_rust::models::{send::commands::SentCommand, shared::Guild};
use openaction::{Instance, OpenActionResult};
use serde::Serialize;
use tokio::sync::RwLock;

#[derive(Serialize, Clone)]
pub struct CachedGuild {
	id: String,
	name: String,
}

pub fn guild_cache() -> &'static RwLock<Vec<CachedGuild>> {
	static CACHE: OnceLock<RwLock<Vec<CachedGuild>>> = OnceLock::new();
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
