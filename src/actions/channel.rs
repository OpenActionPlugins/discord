use crate::client::discord_client;

use std::sync::OnceLock;

use discord_ipc_rust::models::send::commands::{
	GetChannelsArgs, SelectTextChannelArgs, SelectVoiceChannelArgs, SentCommand,
};
use discord_ipc_rust::models::shared::{Channel, ChannelType, Guild};
use openaction::{
	Action, ActionUuid, Instance, OpenActionResult, async_trait, get_instance, visible_instances,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Clone, Copy, PartialEq)]
enum ChannelKind {
	Text,
	Voice,
}

impl ChannelKind {
	fn matches(self, channel_type: &ChannelType) -> bool {
		match self {
			Self::Text => matches!(
				channel_type,
				ChannelType::GuildText
					| ChannelType::GuildAnnouncement
					| ChannelType::AnnouncementThread
					| ChannelType::PublicThread
					| ChannelType::PrivateThread
					| ChannelType::GuildForum
					| ChannelType::GuildMedia
			),
			Self::Voice => matches!(
				channel_type,
				ChannelType::GuildVoice | ChannelType::GuildStageVoice
			),
		}
	}
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum PiRequest {
	RefreshGuilds,
	RequestChannels { guild_id: String },
}

pub fn current_voice_channel() -> &'static RwLock<Option<String>> {
	static CHANNEL: OnceLock<RwLock<Option<String>>> = OnceLock::new();
	CHANNEL.get_or_init(|| RwLock::new(None))
}

#[derive(Serialize, Clone)]
struct CachedGuild {
	id: String,
	name: String,
}

fn guild_cache() -> &'static RwLock<Vec<CachedGuild>> {
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

pub async fn clear_guild_cache() {
	guild_cache().write().await.clear();
}

struct PendingChannelRequest {
	instance_id: String,
	kind: ChannelKind,
}

fn pending_channel_requests() -> &'static RwLock<Vec<PendingChannelRequest>> {
	static PENDING: OnceLock<RwLock<Vec<PendingChannelRequest>>> = OnceLock::new();
	PENDING.get_or_init(|| RwLock::new(Vec::new()))
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ChannelActionSettings {
	pub guild_id: Option<String>,
	pub channel_id: Option<String>,
}

pub async fn send_guilds_to_pi(instance: Option<&Instance>) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct Payload<'a> {
		guilds: &'a [CachedGuild],
	}

	let cache = guild_cache().read().await;
	let payload = Payload { guilds: &cache };

	match instance {
		Some(inst) => inst.send_to_property_inspector(&payload).await,
		None => {
			let instances = visible_instances(TextChannelAction::UUID)
				.await
				.into_iter()
				.chain(visible_instances(VoiceChannelAction::UUID).await);

			for instance in instances {
				let _ = instance.send_to_property_inspector(&payload).await;
			}

			Ok(())
		}
	}
}

pub async fn send_channels_to_pi(channels: &[Channel]) {
	#[derive(Serialize)]
	struct ChannelInfo<'a> {
		id: &'a str,
		name: &'a str,
	}
	#[derive(Serialize)]
	struct Payload<'a> {
		channels: Vec<ChannelInfo<'a>>,
	}

	let pending = std::mem::take(&mut *pending_channel_requests().write().await);
	for req in pending {
		let filtered: Vec<_> = channels
			.iter()
			.filter(|c| req.kind.matches(&c.channel_type))
			.map(|c| ChannelInfo {
				id: &c.id,
				name: c.name.as_deref().unwrap_or(""),
			})
			.collect();

		if let Some(instance) = get_instance(req.instance_id).await {
			let _ = instance
				.send_to_property_inspector(Payload { channels: filtered })
				.await;
		}
	}
}

async fn handle_pi_request(
	instance: &Instance,
	payload: &serde_json::Value,
	kind: ChannelKind,
) -> OpenActionResult<()> {
	let Ok(request) = serde_json::from_value::<PiRequest>(payload.clone()) else {
		return Ok(());
	};

	match request {
		PiRequest::RefreshGuilds => {
			clear_guild_cache().await;
			emit_get_guilds(instance, true).await?;
		}
		PiRequest::RequestChannels { guild_id } => {
			pending_channel_requests()
				.write()
				.await
				.push(PendingChannelRequest {
					instance_id: instance.instance_id.clone(),
					kind,
				});

			let mut lock = discord_client().write().await;
			if let Some(client) = lock.as_mut() {
				if let Err(e) = client
					.emit_command(&SentCommand::GetChannels(GetChannelsArgs { guild_id }))
					.await
				{
					log::error!("Failed to request channels: {}", e);
				}
			}
		}
	}

	Ok(())
}

async fn emit_get_guilds(instance: &Instance, refresh: bool) -> OpenActionResult<()> {
	if !refresh && !guild_cache().read().await.is_empty() {
		return send_guilds_to_pi(Some(instance)).await;
	}

	let mut lock = discord_client().write().await;
	if let Some(client) = lock.as_mut() {
		if let Err(e) = client.emit_command(&SentCommand::GetGuilds).await {
			log::error!("Failed to request guilds: {}", e);
			instance.show_alert().await?;
		}
	}

	Ok(())
}

async fn sync_voice_channel_state(
	instance: &Instance,
	settings: &ChannelActionSettings,
) -> OpenActionResult<()> {
	let is_active = current_voice_channel()
		.read()
		.await
		.as_deref()
		.is_some_and(|ch| settings.channel_id.as_deref() == Some(ch));
	instance.set_state(if is_active { 1 } else { 0 }).await
}

pub struct TextChannelAction;
#[async_trait]
impl Action for TextChannelAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.textchannel";
	type Settings = ChannelActionSettings;

	async fn property_inspector_did_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		emit_get_guilds(instance, false).await
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
		payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		handle_pi_request(instance, payload, ChannelKind::Text).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let Some(channel_id) = settings.channel_id.as_ref() else {
			log::error!("No channel ID configured");
			instance.show_alert().await?;
			return Ok(());
		};

		let mut lock = discord_client().write().await;
		let Some(client) = lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		if let Err(e) = client
			.emit_command(&SentCommand::SelectTextChannel(SelectTextChannelArgs {
				channel_id: Some(channel_id.clone()),
				timeout: None,
			}))
			.await
		{
			log::error!("Failed to select text channel: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}

pub struct VoiceChannelAction;
#[async_trait]
impl Action for VoiceChannelAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.voicechannel";
	type Settings = ChannelActionSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		sync_voice_channel_state(instance, settings).await
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		sync_voice_channel_state(instance, settings).await
	}

	async fn property_inspector_did_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		emit_get_guilds(instance, false).await
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
		payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		handle_pi_request(instance, payload, ChannelKind::Voice).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let mut lock = discord_client().write().await;
		let Some(client) = lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		let Some(channel_id) = settings.channel_id.as_ref() else {
			log::error!("No channel ID configured");
			instance.show_alert().await?;
			return Ok(());
		};

		let target = current_voice_channel()
			.read()
			.await
			.as_deref()
			.filter(|&ch| ch == channel_id)
			.is_none()
			.then(|| channel_id.clone());

		if let Err(e) = client
			.emit_command(&SentCommand::SelectVoiceChannel(SelectVoiceChannelArgs {
				channel_id: target,
				force: Some(true),
				navigate: Some(false),
				timeout: None,
			}))
			.await
		{
			log::error!("Failed to toggle voice channel: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
