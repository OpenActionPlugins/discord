use std::sync::OnceLock;

use discord_ipc_rust::models::{send::commands::SentCommand, shared::Guild};
use openaction::{Instance, OpenActionResult};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::client::discord_client;

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
	*guild_cache().write().await = guilds
		.iter()
		.map(|g| CachedGuild {
			id: g.id.clone(),
			name: g.name.clone(),
		})
		.collect();
}

pub async fn refresh_guild_cache(instance: &Instance) -> OpenActionResult<()> {
    let mut lock = discord_client().write().await;
	if let Some(client) = lock.as_mut() {
		if let Err(e) = client.emit_command(&SentCommand::GetGuilds).await {
			log::error!("Failed to request guilds: {}", e);
			instance.show_alert().await?;
		}
	}

	Ok(())
}
