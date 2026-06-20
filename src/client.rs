use crate::audio_device_utils::{AudioDeviceType, AudioDeviceWrapper, UserVoiceSettings};
use crate::oauth::exchange_code_for_token;
use crate::rpc_events::handle_rpc_event;
use crate::{CURRENT_SETTINGS, DiscordSettings};

use std::collections::HashMap;
use std::sync::{
	LazyLock,
	atomic::{AtomicBool, Ordering},
};

use discord_ipc_rust::DiscordIpcClient;
use discord_ipc_rust::models::receive::{ReceivedItem, commands::ReturnedCommand};
use discord_ipc_rust::models::send::commands::{AuthorizeArgs, SentCommand};
use discord_ipc_rust::models::send::events::SubscribeableEvent;
use discord_ipc_rust::models::shared::voice::VoiceSettingsMode;
use openaction::{Instance, OpenActionResult, set_global_settings};
use tokio::sync::{MappedMutexGuard, Mutex, MutexGuard, RwLock};
use tokio::time::{Duration, sleep};

// Shared place to store the active Discord IPC connection for the lifetime of the plugin.
static DISCORD_CLIENT: LazyLock<Mutex<Option<DiscordIpcClient>>> =
	LazyLock::new(|| Mutex::new(None));

// Shared place to store the user ID of the authenticated user.
pub static CURRENT_USER_ID: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

// Flag to avoid multiple concurrent reconnect attempts.
static RECONNECTING: AtomicBool = AtomicBool::new(false);

pub(super) static AUDIO_INPUT_TYPE: LazyLock<RwLock<Option<AudioDeviceWrapper>>> =
	LazyLock::new(|| RwLock::new(None));

pub(super) static AUDIO_OUTPUT_TYPE: LazyLock<RwLock<Option<AudioDeviceWrapper>>> =
	LazyLock::new(|| RwLock::new(None));

// Shared place to store the currently selected voice channel ID.
pub static CURRENT_VOICE_CHANNEL: LazyLock<RwLock<Option<String>>> =
	LazyLock::new(|| RwLock::new(None));

// Last-known voice mode from Discord, updated via RPC events.
pub static CURRENT_VOICE_MODE: LazyLock<RwLock<Option<VoiceSettingsMode>>> =
	LazyLock::new(|| RwLock::new(None));

pub static USER_VOICE_SETTINGS_MAP: LazyLock<RwLock<HashMap<String, UserVoiceSettings>>> =
	LazyLock::new(|| RwLock::new(HashMap::new()));

// Locks the Discord client and returns the guard, or shows an alert and returns None if not initialized.
pub async fn get_discord_client(
	instance: &Instance,
) -> OpenActionResult<Option<MappedMutexGuard<'static, DiscordIpcClient>>> {
	let guard = DISCORD_CLIENT.lock().await;
	if guard.is_none() {
		log::error!("Discord client not initialized");
		instance.show_alert().await?;
		return Ok(None);
	}
	Ok(Some(MutexGuard::map(guard, |opt| opt.as_mut().unwrap())))
}

pub async fn get_audio_device_settings(
	device_type: &AudioDeviceType,
) -> Option<AudioDeviceWrapper> {
	match device_type {
		AudioDeviceType::Input => &AUDIO_INPUT_TYPE,
		AudioDeviceType::Output => &AUDIO_OUTPUT_TYPE,
	}
	.read()
	.await
	.clone()
}

// Store the latest error message in the global settings so the UI can surface it.
pub async fn update_error(error: &str) {
	let mut current = CURRENT_SETTINGS.write().await;
	if current.error.as_deref() == Some(error) {
		return;
	}
	current.error = Some(error.to_owned());
	if let Err(e) = set_global_settings(&*current).await {
		log::error!("Failed to save error to global settings: {}", e);
	}
}

// Attempts to reinitialize the Discord IPC client using the stored settings.
async fn reinitialize() {
	let settings = CURRENT_SETTINGS.read().await.clone();
	match create_discord_client(&settings).await {
		Ok(client) => {
			*DISCORD_CLIENT.lock().await = Some(client);
			RECONNECTING.store(false, Ordering::SeqCst);
		}
		Err(e) => {
			*DISCORD_CLIENT.lock().await = None;
			log::error!("Failed to reinitialize client: {}", e);
			update_error(&e).await;
		}
	}
}

// Schedules periodic reconnect attempts until successful.
pub(crate) fn schedule_reconnect() {
	if RECONNECTING.swap(true, Ordering::SeqCst) {
		return;
	}

	tokio::spawn(async {
		while RECONNECTING.load(Ordering::SeqCst) {
			reinitialize().await;
			sleep(Duration::from_secs(5)).await;
		}
	});
}

pub async fn update_voice_state_subscription(channel_id: String, subscribe: bool) {
	let mut client_lock = DISCORD_CLIENT.lock().await;
	let Some(client) = client_lock.as_mut() else {
		log::error!("Discord client not initialized");
		return;
	};

	let events = [
		SubscribeableEvent::VoiceStateCreate {
			channel_id: channel_id.clone(),
		},
		SubscribeableEvent::VoiceStateUpdate {
			channel_id: channel_id.clone(),
		},
		SubscribeableEvent::VoiceStateDelete { channel_id },
	];

	for event in events {
		let command = if subscribe {
			SentCommand::Subscribe(event)
		} else {
			SentCommand::Unsubscribe(event)
		};

		if let Err(e) = client.emit_command(&command).await {
			log::error!(
				"Failed to {} voice state events: {}",
				if subscribe {
					"subscribe to"
				} else {
					"unsubscribe from"
				},
				e
			);
		}
	}
}

// Sets up an authenticated Discord IPC client with event subscriptions and handlers.
async fn setup_discord_client(
	rpc: &mut DiscordIpcClient,
	access_token: String,
) -> Result<(), String> {
	rpc.authenticate(access_token)
		.await
		.map_err(|e| format!("Failed to authenticate: {}", e))?;

	// Listen for RPC events and subscribe to voice settings updates.
	rpc.setup_event_handler(move |item| {
		tokio::spawn(async move {
			handle_rpc_event(item).await;
		});
	})
	.await;

	rpc.emit_command(&SentCommand::Subscribe(
		SubscribeableEvent::VoiceSettingsUpdate,
	))
	.await
	.map_err(|e| format!("Failed to subscribe to voice settings updates: {}", e))?;

	rpc.emit_command(&SentCommand::Subscribe(
		SubscribeableEvent::VideoStateUpdate,
	))
	.await
	.map_err(|e| format!("Failed to subscribe to video state updates: {}", e))?;

	rpc.emit_command(&SentCommand::Subscribe(
		SubscribeableEvent::ScreenshareStateUpdate,
	))
	.await
	.map_err(|e| format!("Failed to subscribe to screen share state updates: {}", e))?;

	rpc.emit_command(&SentCommand::Subscribe(
		SubscribeableEvent::VoiceChannelSelect,
	))
	.await
	.map_err(|e| format!("Failed to subscribe to voice channel select events: {}", e))?;

	rpc.emit_command(&SentCommand::Subscribe(
		SubscribeableEvent::NotificationCreate,
	))
	.await
	.map_err(|e| format!("Failed to subscribe to notification creation events: {}", e))?;

	// Request current voice settings so buttons reflect the initial state immediately.
	rpc.emit_command(&SentCommand::GetVoiceSettings)
		.await
		.map_err(|e| format!("Failed to fetch initial voice settings: {}", e))?;

	rpc.emit_command(&SentCommand::GetGuilds)
		.await
		.map_err(|e| format!("Failed to fetch guilds: {}", e))?;

	rpc.emit_command(&SentCommand::GetSelectedVoiceChannel)
		.await
		.map_err(|e| format!("Failed to fetch initially selected voice channel: {}", e))?;

	rpc.emit_command(&SentCommand::GetSoundboardSounds)
		.await
		.map_err(|e| format!("Failed to fetch soundboard sounds: {}", e))?;

	let mut current = CURRENT_SETTINGS.write().await;
	current.error = None;
	if let Err(e) = set_global_settings(&*current).await {
		log::error!("Failed to clear error: {}", e);
	}

	Ok(())
}

// Internal logic that actually connects to Discord and performs OAuth if necessary.
async fn create_discord_client(settings: &DiscordSettings) -> Result<DiscordIpcClient, String> {
	if settings.client_id.is_empty() || settings.client_secret.is_empty() {
		return Err("Client ID or Client Secret not configured".to_owned());
	}

	let (mut rpc, user) = DiscordIpcClient::create(settings.client_id.clone())
		.await
		.map_err(|e| format!("Failed to connect to Discord: {}", e))?;
	log::info!("Connected to Discord as {}", user.username);

	*CURRENT_USER_ID.write().await = Some(user.id);

	if !settings.access_token.is_empty() {
		setup_discord_client(&mut rpc, settings.access_token.clone()).await?;

		Ok(rpc)
	} else {
		log::info!("Starting OAuth authorization flow");

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

			log::info!("Received authorization code, exchanging for access token");
			let client_id = client_id.clone();
			let client_secret = client_secret.clone();

			tokio::spawn(async move {
				match exchange_code_for_token(&code, &client_id, &client_secret).await {
					Ok(access_token) => {
						log::info!("Successfully obtained access token");

						let mut current = CURRENT_SETTINGS.write().await;
						current.access_token = access_token.clone();
						if let Err(e) = set_global_settings(&*current).await {
							log::error!("Failed to save access token: {}", e);
						}
						drop(current);

						let mut client_lock = DISCORD_CLIENT.lock().await;
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
				"rpc.voice.read".to_owned(),
				"rpc.voice.write".to_owned(),
				"rpc.video.read".to_owned(),
				"rpc.video.write".to_owned(),
				"rpc.screenshare.read".to_owned(),
				"rpc.screenshare.write".to_owned(),
				"rpc.notifications.read".to_owned(),
				"identify".to_owned(),
			],
			rpc_token: None,
			username: None,
		}))
		.await
		.map_err(|e| format!("Failed to start authorization: {}", e))?;

		log::info!("Sent authorization request to Discord");
		Ok(rpc)
	}
}
