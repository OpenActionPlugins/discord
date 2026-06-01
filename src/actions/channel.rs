use crate::cache::{CachedGuild, guild_cache, refresh_guild_cache};
use crate::client::{current_voice_channel, discord_client};

use std::collections::HashMap;
use std::sync::OnceLock;

use discord_ipc_rust::models::send::commands::{
	GetChannelsArgs, SelectTextChannelArgs, SelectVoiceChannelArgs, SentCommand,
};
use discord_ipc_rust::models::shared::{Channel, ChannelType};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait, visible_instances};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

fn channel_request_map() -> &'static RwLock<HashMap<String, ChannelKind>> {
	static REQUESTS: OnceLock<RwLock<HashMap<String, ChannelKind>>> = OnceLock::new();
	REQUESTS.get_or_init(|| RwLock::new(HashMap::new()))
}

async fn emit_get_guilds(instance: &Instance, refresh: bool) -> OpenActionResult<()> {
	if !refresh && !guild_cache().read().await.is_empty() {
		return send_guilds_to_pi(Some(instance)).await;
	}

	refresh_guild_cache(instance).await
}

async fn get_all_instances() -> impl Iterator<Item = Arc<Instance>> {
	visible_instances(TextChannelAction::UUID)
		.await
		.into_iter()
		.chain(visible_instances(VoiceChannelAction::UUID).await)
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
			for inst in get_all_instances().await {
				let _ = inst.send_to_property_inspector(&payload).await;
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

	let mut requests = channel_request_map().write().await;

	for instance in get_all_instances().await {
		if let Some(kind) = requests.remove(&instance.instance_id) {
			let mut filtered: Vec<_> = channels
				.iter()
				.filter(|c| kind.matches(&c.channel_type))
				.map(|c| ChannelInfo {
					id: &c.id,
					name: c.name.as_deref().unwrap_or(""),
				})
				.collect();
			filtered.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

			let _ = instance
				.send_to_property_inspector(Payload { channels: filtered })
				.await;
		}
	}
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum PiRequest {
	RefreshGuilds,
	RequestChannels { guild_id: String },
}

impl PiRequest {
	async fn handle(
		instance: &Instance,
		payload: &serde_json::Value,
		kind: ChannelKind,
	) -> OpenActionResult<()> {
		let Ok(request) = serde_json::from_value(payload.clone()) else {
			return Ok(());
		};

		match request {
			PiRequest::RefreshGuilds => {
				emit_get_guilds(instance, true).await?;
			}
			PiRequest::RequestChannels { guild_id } => {
				channel_request_map()
					.write()
					.await
					.insert(instance.instance_id.clone(), kind);

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
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ChannelActionSettings {
	pub guild_id: Option<String>,
	pub channel_id: Option<String>,
}

impl ChannelActionSettings {
	fn check_valid(&self) -> Option<&str> {
		self.guild_id.as_deref().filter(|s| !s.is_empty())?;
		self.channel_id.as_deref().filter(|s| !s.is_empty())
	}
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
		PiRequest::handle(instance, payload, ChannelKind::Text).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let Some(channel_id) = settings.check_valid() else {
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
				channel_id: Some(channel_id.to_string()),
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
		PiRequest::handle(instance, payload, ChannelKind::Voice).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let mut lock = discord_client().write().await;
		let Some(client) = lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		let Some(channel_id) = settings.check_valid() else {
			instance.show_alert().await?;
			return Ok(());
		};

		let target = current_voice_channel()
			.read()
			.await
			.as_deref()
			.filter(|&ch| ch == channel_id)
			.is_none()
			.then(|| channel_id.to_string());

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
