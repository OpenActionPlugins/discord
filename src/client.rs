use crate::oauth::exchange_code_for_token;
use crate::rpc_events::handle_rpc_event;
use crate::{DiscordSettings, current_settings};

use std::sync::{
	OnceLock,
	atomic::{AtomicBool, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

use discord_ipc_rust::DiscordIpcClient;
use discord_ipc_rust::models::receive::{
	ReceivedItem,
	commands::{GetGuildData, ReturnedCommand},
};
use discord_ipc_rust::models::send::commands::{
	AuthorizeArgs, GetChannelsArgs, GetGuildArgs, SelectTextChannelArgs, SelectVoiceChannelArgs, SentCommand,
	SetVoiceSettingsArgs,
};
use discord_ipc_rust::models::send::events::SubscribeableEvent;
use discord_ipc_rust::models::shared::{Channel, Guild, voice::VoiceSettings};
use openaction::set_global_settings;
use serde_json::json;
use tokio::sync::{Mutex, RwLock, oneshot};
use tokio::time::{Duration, sleep, timeout};

const SELECTED_VOICE_CHANNEL_POLL_SECS: u64 = 8;
const DEFAULT_RPC_TIMEOUT_SECS: u64 = 3;
const DISCOVERY_RPC_TIMEOUT_SECS: u64 = 10;

pub fn discord_client() -> &'static RwLock<Option<DiscordIpcClient>> {
	static CLIENT: OnceLock<RwLock<Option<DiscordIpcClient>>> = OnceLock::new();
	CLIENT.get_or_init(|| RwLock::new(None))
}

#[derive(Default)]
pub struct DiscordRuntimeState {
	pub connected: bool,
	pub voice_settings: Option<VoiceSettings>,
	pub selected_voice_channel: Option<Channel>,
	pub selected_voice_channel_id: Option<String>,
	pub selected_voice_guild_id: Option<String>,
}

pub fn discord_state() -> &'static RwLock<DiscordRuntimeState> {
	static STATE: OnceLock<RwLock<DiscordRuntimeState>> = OnceLock::new();
	STATE.get_or_init(|| RwLock::new(DiscordRuntimeState::default()))
}

pub async fn set_connected(connected: bool) {
	discord_state().write().await.connected = connected;
}

pub async fn is_connected() -> bool {
	discord_state().read().await.connected
}

pub async fn connect() -> Result<(), String> {
	let settings = current_settings().read().await.clone();
	let client = create_discord_client(&settings).await?;
	*discord_client().write().await = Some(client);
	reconnecting_flag().store(false, Ordering::SeqCst);
	set_connected(true).await;
	ensure_selected_voice_channel_poller();
	Ok(())
}

#[allow(dead_code)]
pub async fn disconnect() {
	*discord_client().write().await = None;
	set_connected(false).await;
}

pub async fn update_voice_settings_cache(settings: VoiceSettings) {
	discord_state().write().await.voice_settings = Some(settings);
}

pub async fn update_selected_voice_channel_cache(channel: Option<&Channel>) {
	let mut state = discord_state().write().await;
	state.selected_voice_channel = channel.and_then(copy_channel);
	state.selected_voice_channel_id = channel.map(|value| value.id.clone());
	state.selected_voice_guild_id = channel.and_then(|value| value.guild_id.clone());
}

pub async fn update_selected_voice_channel_ids(
	channel_id: Option<String>,
	guild_id: Option<String>,
) {
	let mut state = discord_state().write().await;
	if state.selected_voice_channel_id != channel_id {
		state.selected_voice_channel = None;
	}
	state.selected_voice_channel_id = channel_id;
	state.selected_voice_guild_id = guild_id;
}

pub async fn current_selected_voice_channel_id() -> Option<String> {
	discord_state().read().await.selected_voice_channel_id.clone()
}

pub async fn current_selected_voice_channel() -> Option<Channel> {
	discord_state().read().await.selected_voice_channel.as_ref().and_then(copy_channel)
}

pub async fn update_error(error: &str) {
	let mut current = current_settings().write().await;
	if current.error.as_deref() == Some(error) {
		return;
	}
	current.error = Some(error.to_owned());
	if let Err(e) = set_global_settings(&*current).await {
		log::error!("Failed to save error to global settings: {}", e);
	}
}

fn reconnecting_flag() -> &'static AtomicBool {
	static RECONNECTING: OnceLock<AtomicBool> = OnceLock::new();
	RECONNECTING.get_or_init(|| AtomicBool::new(false))
}

fn selected_voice_channel_poller_started() -> &'static AtomicBool {
	static POLLER_STARTED: OnceLock<AtomicBool> = OnceLock::new();
	POLLER_STARTED.get_or_init(|| AtomicBool::new(false))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingRequestKind {
	GetVoiceSettings,
	SetVoiceSettings,
	GetGuild,
	GetGuilds,
	GetChannels,
	SelectVoiceChannel,
	GetSelectedVoiceChannel,
	SelectTextChannel,
}

enum PendingResponse {
	VoiceSettings(VoiceSettings),
	Guild(GetGuildData),
	#[allow(dead_code)]
	Guilds(Vec<Guild>),
	#[allow(dead_code)]
	Channels(Vec<Channel>),
	Channel(Option<Channel>),
}

struct PendingRequest {
	kind: PendingRequestKind,
	sender: oneshot::Sender<Result<PendingResponse, String>>,
}

fn pending_request() -> &'static Mutex<Option<PendingRequest>> {
	static PENDING: OnceLock<Mutex<Option<PendingRequest>>> = OnceLock::new();
	PENDING.get_or_init(|| Mutex::new(None))
}

fn request_lock() -> &'static Mutex<()> {
	static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
	LOCK.get_or_init(|| Mutex::new(()))
}

fn copy_voice_settings(settings: &VoiceSettings) -> Option<VoiceSettings> {
	serde_json::to_value(settings)
		.ok()
		.and_then(|value| serde_json::from_value(value).ok())
}

fn copy_channel(channel: &Channel) -> Option<Channel> {
	serde_json::to_value(channel)
		.ok()
		.and_then(|value| serde_json::from_value(value).ok())
}

fn copy_guild(guild: &Guild) -> Option<Guild> {
	serde_json::to_value(guild)
		.ok()
		.and_then(|value| serde_json::from_value(value).ok())
}

fn command_label(kind: PendingRequestKind) -> &'static str {
	match kind {
		PendingRequestKind::GetVoiceSettings => "GET_VOICE_SETTINGS",
		PendingRequestKind::SetVoiceSettings => "SET_VOICE_SETTINGS",
		PendingRequestKind::GetGuild => "GET_GUILD",
		PendingRequestKind::GetGuilds => "GET_GUILDS",
		PendingRequestKind::GetChannels => "GET_CHANNELS",
		PendingRequestKind::SelectVoiceChannel => "SELECT_VOICE_CHANNEL",
		PendingRequestKind::GetSelectedVoiceChannel => "GET_SELECTED_VOICE_CHANNEL",
		PendingRequestKind::SelectTextChannel => "SELECT_TEXT_CHANNEL",
	}
}

fn command_timeout(kind: PendingRequestKind) -> Duration {
	match kind {
		PendingRequestKind::GetGuild
		| PendingRequestKind::GetGuilds
		| PendingRequestKind::GetChannels => {
			Duration::from_secs(DISCOVERY_RPC_TIMEOUT_SECS)
		}
		_ => Duration::from_secs(DEFAULT_RPC_TIMEOUT_SECS),
	}
}

pub async fn fulfill_pending_command(command: &ReturnedCommand) -> bool {
	let kind = match command {
		ReturnedCommand::GetVoiceSettings(_) => PendingRequestKind::GetVoiceSettings,
		ReturnedCommand::SetVoiceSettings(_) => PendingRequestKind::SetVoiceSettings,
		ReturnedCommand::GetGuild(_) => PendingRequestKind::GetGuild,
		ReturnedCommand::GetGuilds(_) => PendingRequestKind::GetGuilds,
		ReturnedCommand::GetChannels(_) => PendingRequestKind::GetChannels,
		ReturnedCommand::SelectVoiceChannel(_) => PendingRequestKind::SelectVoiceChannel,
		ReturnedCommand::GetSelectedVoiceChannel(_) => PendingRequestKind::GetSelectedVoiceChannel,
		ReturnedCommand::SelectTextChannel(_) => PendingRequestKind::SelectTextChannel,
		_ => return false,
	};

	let mut pending = pending_request().lock().await;
	let Some(request) = pending.take() else {
		return false;
	};

	if request.kind != kind {
		*pending = Some(request);
		return false;
	}

	let response = match command {
		ReturnedCommand::GetVoiceSettings(settings)
		| ReturnedCommand::SetVoiceSettings(settings) => {
			let Some(settings) = copy_voice_settings(settings) else {
				return false;
			};
			PendingResponse::VoiceSettings(settings)
		}
		ReturnedCommand::GetGuild(guild) => PendingResponse::Guild(GetGuildData {
			id: guild.id.clone(),
			name: guild.name.clone(),
			icon_url: guild.icon_url.clone(),
		}),
		ReturnedCommand::GetGuilds(guilds) => PendingResponse::Guilds(
			guilds.iter().filter_map(copy_guild).collect(),
		),
		ReturnedCommand::GetChannels(channels) => PendingResponse::Channels(
			channels.iter().filter_map(copy_channel).collect(),
		),
		ReturnedCommand::SelectVoiceChannel(channel)
		| ReturnedCommand::GetSelectedVoiceChannel(channel)
		| ReturnedCommand::SelectTextChannel(channel) => PendingResponse::Channel(
			channel.as_ref().and_then(copy_channel),
		),
		_ => return false,
	};

	log::debug!("Discord RPC response received for {}", command_label(kind));
	let _ = request.sender.send(Ok(response));
	true
}

pub async fn fulfill_pending_error(code: u32, message: String) -> bool {
	let mut pending = pending_request().lock().await;
	let Some(request) = pending.take() else {
		return false;
	};

	let error = format!("Discord RPC error {}: {}", code, message);
	log::warn!(
		"Discord RPC request {} failed: {}",
		command_label(request.kind),
		error
	);
	let _ = request.sender.send(Err(error));
	true
}

enum OutgoingRequest {
	Command(SentCommand),
	RawJson(String),
}

fn timeout_error_for(kind: PendingRequestKind) -> String {
	format!(
		"Timed out waiting for Discord RPC response to {}",
		command_label(kind)
	)
}

fn add_nonce_to_payload(payload: &str) -> Result<String, String> {
	let mut value: serde_json::Value =
		serde_json::from_str(payload).map_err(|error| format!("Invalid RPC payload JSON: {}", error))?;
	let object = value
		.as_object_mut()
		.ok_or_else(|| "RPC payload must be a JSON object".to_owned())?;

	let nonce = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|value| value.as_nanos().to_string())
		.unwrap_or_else(|_| "0".to_owned());
	object.insert("nonce".to_owned(), serde_json::Value::String(nonce));

	serde_json::to_string(&value).map_err(|error| format!("Failed to serialize RPC payload: {}", error))
}

async fn send_request(
	request: OutgoingRequest,
	kind: PendingRequestKind,
) -> Result<PendingResponse, String> {
	let _guard = request_lock().lock().await;
	let (sender, receiver) = oneshot::channel();
	*pending_request().lock().await = Some(PendingRequest { kind, sender });
	log::debug!("Sending Discord RPC request {}", command_label(kind));
	let request_timeout = command_timeout(kind);

	{
		let mut client_lock = discord_client().write().await;
		let Some(client) = client_lock.as_mut() else {
			*pending_request().lock().await = None;
			return Err("Discord IPC client not initialized. Is Discord running?".to_owned());
		};

		let send_result = match request {
			OutgoingRequest::Command(command) => client.emit_command(&command).await,
			OutgoingRequest::RawJson(payload) => {
				let payload = add_nonce_to_payload(&payload)?;
				client.emit_string(&payload).await
			}
		};

		if let Err(error) = send_result {
			*pending_request().lock().await = None;
			return Err(format!("Failed to send Discord RPC command: {}", error));
		}
	}

	match timeout(request_timeout, receiver).await {
		Ok(Ok(Ok(response))) => Ok(response),
		Ok(Ok(Err(error))) => Err(error),
		Ok(Err(_)) => {
			*pending_request().lock().await = None;
			Err("Discord RPC response channel closed unexpectedly".to_owned())
		}
		Err(_) => {
			*pending_request().lock().await = None;
			log::warn!(
				"Timed out waiting for Discord RPC response to {} after {} seconds",
				command_label(kind),
				request_timeout.as_secs()
			);
			Err(timeout_error_for(kind))
		}
	}
}

pub async fn request_voice_settings() -> Result<VoiceSettings, String> {
	log::info!("Fetching Discord voice settings");
	match send_request(
		OutgoingRequest::Command(SentCommand::GetVoiceSettings),
		PendingRequestKind::GetVoiceSettings,
	)
	.await?
	{
		PendingResponse::VoiceSettings(settings) => Ok(settings),
		_ => Err("Discord returned an unexpected response".to_owned()),
	}
}

pub async fn set_voice_settings(args: SetVoiceSettingsArgs) -> Result<VoiceSettings, String> {
	log::info!("Updating Discord voice settings");
	match send_request(
		OutgoingRequest::Command(SentCommand::SetVoiceSettings(args)),
		PendingRequestKind::SetVoiceSettings,
	)
	.await?
	{
		PendingResponse::VoiceSettings(settings) => Ok(settings),
		_ => Err("Discord returned an unexpected response".to_owned()),
	}
}

pub async fn set_voice_settings_json(args: serde_json::Value) -> Result<VoiceSettings, String> {
	log::info!("Updating Discord voice settings with raw JSON payload");
	match send_request(
		OutgoingRequest::RawJson(
			json!({
				"cmd": "SET_VOICE_SETTINGS",
				"args": args
			})
			.to_string(),
		),
		PendingRequestKind::SetVoiceSettings,
	)
	.await?
	{
		PendingResponse::VoiceSettings(settings) => Ok(settings),
		_ => Err("Discord returned an unexpected response".to_owned()),
	}
}

#[allow(dead_code)]
pub async fn get_guilds() -> Result<Vec<Guild>, String> {
	log::info!("Fetching Discord guilds");
	let primary = send_request(
		OutgoingRequest::Command(SentCommand::GetGuilds),
		PendingRequestKind::GetGuilds,
	)
	.await;

	let response = match primary {
		Ok(response) => response,
		Err(error) if error == timeout_error_for(PendingRequestKind::GetGuilds) => {
			log::warn!("GET_GUILDS timed out; retrying with raw RPC payload");
			send_request(
				OutgoingRequest::RawJson(
					json!({
						"cmd": "GET_GUILDS",
						"args": {}
					})
					.to_string(),
				),
				PendingRequestKind::GetGuilds,
			)
			.await?
		}
		Err(error) => return Err(error),
	};

	match response {
		PendingResponse::Guilds(guilds) => Ok(guilds),
		_ => Err("Discord returned an unexpected response".to_owned()),
	}
}

pub async fn get_guild(guild_id: String) -> Result<GetGuildData, String> {
	log::info!("Fetching Discord guild {}", guild_id);
	match send_request(
		OutgoingRequest::Command(SentCommand::GetGuild(GetGuildArgs {
			guild_id,
			timeout: Some(10),
		})),
		PendingRequestKind::GetGuild,
	)
	.await?
	{
		PendingResponse::Guild(guild) => Ok(guild),
		_ => Err("Discord returned an unexpected response".to_owned()),
	}
}

#[allow(dead_code)]
pub async fn get_channels(guild_id: String) -> Result<Vec<Channel>, String> {
	log::info!("Fetching Discord channels for guild {}", guild_id);
	let primary = send_request(
		OutgoingRequest::Command(SentCommand::GetChannels(GetChannelsArgs {
			guild_id: guild_id.clone(),
		})),
		PendingRequestKind::GetChannels,
	)
	.await;

	let response = match primary {
		Ok(response) => response,
		Err(error) if error == timeout_error_for(PendingRequestKind::GetChannels) => {
			log::warn!("GET_CHANNELS timed out; retrying with raw RPC payload");
			send_request(
				OutgoingRequest::RawJson(
					json!({
						"cmd": "GET_CHANNELS",
						"args": {
							"guild_id": guild_id
						}
					})
					.to_string(),
				),
				PendingRequestKind::GetChannels,
			)
			.await?
		}
		Err(error) => return Err(error),
	};

	match response {
		PendingResponse::Channels(channels) => Ok(channels),
		_ => Err("Discord returned an unexpected response".to_owned()),
	}
}

pub async fn get_selected_voice_channel() -> Result<Option<Channel>, String> {
	log::debug!("Fetching selected Discord voice channel");
	match send_request(
		OutgoingRequest::Command(SentCommand::GetSelectedVoiceChannel),
		PendingRequestKind::GetSelectedVoiceChannel,
	)
	.await?
	{
		PendingResponse::Channel(channel) => Ok(channel),
		_ => Err("Discord returned an unexpected response".to_owned()),
	}
}

pub async fn select_voice_channel(
	channel_id: Option<String>,
	force: Option<bool>,
	navigate: Option<bool>,
) -> Result<Option<Channel>, String> {
	log::info!(
		"Selecting Discord voice channel: channel_id={:?}, force={:?}, navigate={:?}",
		channel_id,
		force,
		navigate
	);
	match send_request(
		OutgoingRequest::Command(SentCommand::SelectVoiceChannel(SelectVoiceChannelArgs {
			channel_id,
			timeout: Some(3),
			force,
			navigate,
		})),
		PendingRequestKind::SelectVoiceChannel,
	)
	.await?
	{
		PendingResponse::Channel(channel) => Ok(channel),
		_ => Err("Discord returned an unexpected voice settings payload".to_owned()),
	}
}

pub async fn leave_voice_channel() -> Result<Option<Channel>, String> {
	log::info!("Leaving current Discord voice channel");
	select_voice_channel(None, None, None).await
}

pub async fn select_text_channel(channel_id: Option<String>) -> Result<Option<Channel>, String> {
	log::info!("Selecting Discord text channel: channel_id={:?}", channel_id);
	match send_request(
		OutgoingRequest::Command(SentCommand::SelectTextChannel(SelectTextChannelArgs {
			channel_id,
			timeout: Some(3),
		})),
		PendingRequestKind::SelectTextChannel,
	)
	.await?
	{
		PendingResponse::Channel(channel) => Ok(channel),
		_ => Err("Discord returned an unexpected voice settings payload".to_owned()),
	}
}

fn ensure_selected_voice_channel_poller() {
	let started = selected_voice_channel_poller_started();
	if started.swap(true, Ordering::SeqCst) {
		return;
	}

	tokio::spawn(async move {
		loop {
			if is_connected().await {
				match get_selected_voice_channel().await {
					Ok(_) => crate::actions::sync_join_voice_channel_states().await,
					Err(error) => {
						log::debug!("Selected voice channel poll skipped: {}", error);
					}
				}
			}
			sleep(Duration::from_secs(SELECTED_VOICE_CHANNEL_POLL_SECS)).await;
		}
	});
}

async fn reinitialize() {
	match connect().await {
		Ok(()) => {}
		Err(e) => {
			*discord_client().write().await = None;
			set_connected(false).await;
			log::error!("Failed to reinitialize client: {}", e);
			update_error(&e).await;
		}
	}
}

pub(crate) fn schedule_reconnect() {
	let flag = reconnecting_flag();
	if flag.swap(true, Ordering::SeqCst) {
		return;
	}

	tokio::spawn(async move {
		while flag.load(Ordering::SeqCst) {
			reinitialize().await;
			sleep(Duration::from_secs(5)).await;
		}
	});
}

async fn setup_discord_client(
	rpc: &mut DiscordIpcClient,
	access_token: String,
) -> Result<(), String> {
	log::info!("Authenticating Discord RPC client");
	rpc.authenticate(access_token)
		.await
		.map_err(|e| format!("Failed to authenticate: {}", e))?;

	rpc.setup_event_handler(move |item| {
		tokio::spawn(async move {
			handle_rpc_event(item).await;
		});
	})
	.await;

	log::info!("Subscribing to Discord voice RPC events");
	rpc.emit_command(&SentCommand::Subscribe(
		SubscribeableEvent::VoiceSettingsUpdate,
	))
	.await
	.map_err(|e| format!("Failed to subscribe to voice updates: {}", e))?;
	rpc.emit_command(&SentCommand::Subscribe(
		SubscribeableEvent::VoiceChannelSelect,
	))
	.await
	.map_err(|e| format!("Failed to subscribe to voice channel selection updates: {}", e))?;

	rpc.emit_command(&SentCommand::GetVoiceSettings)
		.await
		.map_err(|e| format!("Failed to fetch initial voice settings: {}", e))?;
	rpc.emit_command(&SentCommand::GetSelectedVoiceChannel)
		.await
		.map_err(|e| format!("Failed to fetch selected voice channel: {}", e))?;

	set_connected(true).await;
	ensure_selected_voice_channel_poller();

	let mut current = current_settings().write().await;
	current.error = None;
	if let Err(e) = set_global_settings(&*current).await {
		log::error!("Failed to clear error: {}", e);
	}

	Ok(())
}

async fn create_discord_client(settings: &DiscordSettings) -> Result<DiscordIpcClient, String> {
	if settings.client_id.is_empty() || settings.client_secret.is_empty() {
		return Err("Client ID or Client Secret not configured".to_owned());
	}

	log::info!("Connecting to Discord local RPC socket");
	let (mut rpc, user) = DiscordIpcClient::create(settings.client_id.clone())
		.await
		.map_err(|e| format!("Failed to connect to Discord: {}", e))?;
	log::info!("Connected to Discord as {}", user.username);

	if !settings.access_token.is_empty() {
		setup_discord_client(&mut rpc, settings.access_token.clone()).await?;
		Ok(rpc)
	} else {
		log::info!("Starting Discord OAuth authorization flow");

		let client_id = settings.client_id.clone();
		let client_secret = settings.client_secret.clone();

		rpc.setup_event_handler(move |item| {
			let code = match &item {
				ReceivedItem::Command(command) => match &**command {
					ReturnedCommand::Authorize { code } => Some(code.clone()),
					_ => None,
				},
				_ => None,
			};

			let Some(code) = code else {
				tokio::spawn(async move {
					handle_rpc_event(item).await;
				});
				return;
			};

			log::info!("Received Discord authorization code");
			let client_id = client_id.clone();
			let client_secret = client_secret.clone();

			tokio::spawn(async move {
				match exchange_code_for_token(&code, &client_id, &client_secret).await {
					Ok(access_token) => {
						log::info!("Successfully obtained Discord access token");

						let mut current = current_settings().write().await;
						current.access_token = access_token.clone();
						if let Err(e) = set_global_settings(&*current).await {
							log::error!("Failed to save access token: {}", e);
						}
						drop(current);

						let mut client_lock = discord_client().write().await;
						let Some(client) = client_lock.as_mut() else {
							log::error!("Discord client not initialized");
							return;
						};

						client.remove_event_handler();
						if let Err(error) = setup_discord_client(client, access_token).await {
							let error_msg =
								format!("Failed to set up authenticated client: {}", error);
							log::error!("{}", error_msg);
							update_error(&error_msg).await;
						}
					}
					Err(e) => {
						let error_msg = format!("Failed to exchange code for token: {}", e);
						log::error!("{}", error_msg);
						update_error(&error_msg).await;
					}
				}
			});
		})
		.await;

		rpc.emit_command(&SentCommand::Authorize(AuthorizeArgs {
			client_id: settings.client_id.clone(),
			scopes: vec![
				"rpc".to_owned(),
				"identify".to_owned(),
				"rpc.voice.read".to_owned(),
				"rpc.voice.write".to_owned(),
			],
			rpc_token: None,
			username: None,
		}))
		.await
		.map_err(|e| format!("Failed to start authorization: {}", e))?;

		log::info!("Sent Discord authorization request");
		Ok(rpc)
	}
}
