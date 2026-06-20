use crate::client::get_discord_client;

use std::sync::LazyLock;

use discord_ipc_rust::models::{send::commands::SentCommand, shared::Guild};
use openaction::{Instance, OpenActionResult};
use serde::Serialize;
use tokio::sync::RwLock;

#[derive(Serialize, Clone)]
pub struct CachedGuild {
	id: String,
	name: String,
}

pub static GUILD_CACHE: LazyLock<RwLock<Vec<CachedGuild>>> =
	LazyLock::new(|| RwLock::new(Vec::new()));

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
