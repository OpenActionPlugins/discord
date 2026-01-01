use crate::oauth::exchange_code_for_token;
use crate::rpc_events::handle_rpc_event;
use crate::{DiscordSettings, current_settings};

use std::sync::{
	OnceLock,
	atomic::{AtomicBool, Ordering},
};

use discord_ipc_rust::DiscordIpcClient;
use discord_ipc_rust::models::receive::{ReceivedItem, commands::ReturnedCommand};
use discord_ipc_rust::models::send::commands::{AuthorizeArgs, SentCommand};
use discord_ipc_rust::models::send::events::SubscribeableEvent;
use openaction::set_global_settings;
use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};

// Shared place to store the active Discord IPC connection for the lifetime of the plugin.
pub fn discord_client() -> &'static RwLock<Option<DiscordIpcClient>> {
	static CLIENT: OnceLock<RwLock<Option<DiscordIpcClient>>> = OnceLock::new();
	CLIENT.get_or_init(|| RwLock::new(None))
}

// Store the latest error message in the global settings so the UI can surface it.
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

// Flag to avoid multiple concurrent reconnect attempts.
fn reconnecting_flag() -> &'static AtomicBool {
	static RECONNECTING: OnceLock<AtomicBool> = OnceLock::new();
	RECONNECTING.get_or_init(|| AtomicBool::new(false))
}

// Attempts to reinitialize the Discord IPC client using the stored settings.
async fn reinitialize() {
	let settings = current_settings().read().await.clone();
	match create_discord_client(&settings).await {
		Ok(client) => {
			*discord_client().write().await = Some(client);
			reconnecting_flag().store(false, Ordering::SeqCst);
		}
		Err(e) => {
			*discord_client().write().await = None;
			log::error!("Failed to reinitialize client: {}", e);
			update_error(&e).await;
		}
	}
}

// Schedules periodic reconnect attempts until successful.
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
	.map_err(|e| format!("Failed to subscribe to voice updates: {}", e))?;

	// Request current voice settings so buttons reflect the initial state immediately.
	rpc.emit_command(&SentCommand::GetVoiceSettings)
		.await
		.map_err(|e| format!("Failed to fetch initial voice settings: {}", e))?;

	let mut current = current_settings().write().await;
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
			scopes: vec!["rpc".to_owned(), "identify".to_owned()],
			rpc_token: None,
			username: None,
		}))
		.await
		.map_err(|e| format!("Failed to start authorization: {}", e))?;

		log::info!("Sent authorization request to Discord");
		Ok(rpc)
	}
}
