use crate::actions::PluginActionSettings;
use crate::client::{
	current_selected_voice_channel, current_selected_voice_channel_id, get_guild, is_connected,
	leave_voice_channel, select_text_channel, select_voice_channel, update_error,
};

use base64::Engine as _;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use discord_ipc_rust::models::shared::Channel;
use openaction::{
	Action, ActionUuid, Instance, OpenActionResult, async_trait, visible_instances,
};
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, sleep};

#[derive(Clone, Default)]
struct JoinVoicePressState {
	press_id: u64,
	handled_by_hold: bool,
}

fn guild_icon_cache() -> &'static RwLock<HashMap<String, Option<String>>> {
	static CACHE: OnceLock<RwLock<HashMap<String, Option<String>>>> = OnceLock::new();
	CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn join_voice_settings() -> &'static RwLock<HashMap<String, PluginActionSettings>> {
	static SETTINGS: OnceLock<RwLock<HashMap<String, PluginActionSettings>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn join_voice_press_states() -> &'static RwLock<HashMap<String, Arc<Mutex<JoinVoicePressState>>>> {
	static STATES: OnceLock<RwLock<HashMap<String, Arc<Mutex<JoinVoicePressState>>>>> =
		OnceLock::new();
	STATES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn instance_title(settings: &PluginActionSettings) -> Option<String> {
	settings
		.trimmed_channel_name()
		.or_else(|| settings.trimmed_channel_id())
}

fn voice_state_count(channel: &Channel) -> usize {
	channel
		.voice_states
		.as_ref()
		.map(|states| states.len())
		.unwrap_or(0)
}

async fn guild_icon_data_url(icon_url: &str) -> Option<String> {
	if let Some(cached) = guild_icon_cache().read().await.get(icon_url).cloned() {
		return cached;
	}

	let response = match reqwest::get(icon_url).await {
		Ok(response) => response,
		Err(error) => {
			log::warn!("Failed to fetch guild icon {}: {}", icon_url, error);
			guild_icon_cache()
				.write()
				.await
				.insert(icon_url.to_owned(), None);
			return None;
		}
	};

	let content_type = response
		.headers()
		.get(reqwest::header::CONTENT_TYPE)
		.and_then(|value| value.to_str().ok())
		.unwrap_or("image/png")
		.to_owned();

	let bytes = match response.bytes().await {
		Ok(bytes) => bytes,
		Err(error) => {
			log::warn!("Failed to read guild icon {}: {}", icon_url, error);
			guild_icon_cache()
				.write()
				.await
				.insert(icon_url.to_owned(), None);
			return None;
		}
	};

	let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
	let data_url = format!("data:{};base64,{}", content_type, encoded);
	guild_icon_cache()
		.write()
		.await
		.insert(icon_url.to_owned(), Some(data_url.clone()));
	Some(data_url)
}

async fn fail(instance: &Instance, message: &str) -> OpenActionResult<()> {
	log::error!("{}", message);
	update_error(message).await;
	instance.show_alert().await?;
	Ok(())
}

async fn send_pi_payload(instance: &Instance, payload: Value) {
	if let Err(error) = instance.send_to_property_inspector(payload).await {
		log::warn!("Failed to send property inspector payload: {}", error);
	}
}

async fn send_pi_status(instance: &Instance, status: &str, warning: Option<&str>) {
	send_pi_payload(
		instance,
		json!({
			"type": "voiceChannel/status",
			"status": status,
			"warning": warning,
		}),
	)
	.await;
}

async fn sync_join_voice_instance(instance: &Instance, settings: &PluginActionSettings) {
	let configured_channel_id = settings.trimmed_channel_id();
	let current_channel_id = current_selected_voice_channel_id().await;
	let active = configured_channel_id.is_some() && configured_channel_id == current_channel_id;
	let selected_channel = current_selected_voice_channel().await;
	let voice_count = if active {
		selected_channel
			.as_ref()
			.map(voice_state_count)
			.filter(|count| *count > 0)
	} else {
		None
	};

	let title = if settings.show_user_count {
		match (instance_title(settings), voice_count) {
			(Some(channel_name), Some(count)) => Some(format!("{}\n{} in call", channel_name, count)),
			(Some(channel_name), None) => Some(channel_name),
			(None, Some(count)) => Some(format!("{} in call", count)),
			(None, None) => None,
		}
	} else {
		instance_title(settings)
	};

	if let Err(error) = instance.set_title(title, None).await {
		log::warn!("Failed to set join voice title: {}", error);
	}

	if !active {
		if let Some(icon_url) = settings.trimmed_guild_icon_url() {
			if let Some(data_url) = guild_icon_data_url(&icon_url).await {
				if let Err(error) = instance.set_image(Some(data_url), Some(0)).await {
					log::warn!("Failed to set guild icon image: {}", error);
				}
			}
		} else if let Err(error) = instance.set_image(None::<String>, Some(0)).await {
			log::warn!("Failed to clear inactive-state custom image: {}", error);
		}
	} else if let Err(error) = instance.set_image(None::<String>, Some(1)).await {
		log::warn!("Failed to clear active-state custom image: {}", error);
	}

	if settings.show_active_state {
		let state = if active { 1 } else { 0 };
		if let Err(error) = instance.set_state(state).await {
			log::warn!("Failed to set join voice state: {}", error);
		}
	}
}

async fn seed_or_explain_join_voice_settings(
	instance: &Instance,
	settings: &PluginActionSettings,
) -> OpenActionResult<()> {
	if settings.trimmed_channel_id().is_some() {
		send_pi_status(
			instance,
			"Use Current Voice Channel to replace this button's saved voice channel, or edit the manual IDs below.",
			None,
		)
		.await;
		return Ok(());
	}

	if !is_connected().await {
		send_pi_status(
			instance,
			"Discord RPC is not connected. Start Discord and re-save your credentials if needed.",
			None,
		)
		.await;
		return Ok(());
	}

	if current_selected_voice_channel().await.is_some() {
		use_current_voice_channel(instance, settings).await
	} else {
		send_pi_status(
			instance,
			"Join the target Discord voice channel, then click Use Current Voice Channel. Manual Guild ID and Channel ID are still available below if needed.",
			None,
		)
		.await;
		Ok(())
	}
}

async fn store_join_voice_instance(instance: &Instance, settings: &PluginActionSettings) {
	join_voice_settings()
		.write()
		.await
		.insert(instance.instance_id.clone(), settings.clone());
	join_voice_press_states()
		.write()
		.await
		.entry(instance.instance_id.clone())
		.or_insert_with(|| Arc::new(Mutex::new(JoinVoicePressState::default())));
}

async fn hold_state_for(instance_id: &str) -> Option<Arc<Mutex<JoinVoicePressState>>> {
	let states = join_voice_press_states().read().await;
	states.get(instance_id).cloned()
}

pub async fn sync_join_voice_channel_states() {
	let settings_map = join_voice_settings().read().await.clone();
	for instance in visible_instances(JoinVoiceChannelAction::UUID).await {
		let Some(settings) = settings_map.get(&instance.instance_id).cloned() else {
			continue;
		};
		sync_join_voice_instance(&instance, &settings).await;
	}
}

async fn handle_join_voice_key_up(
	instance: &Instance,
	settings: &PluginActionSettings,
) -> OpenActionResult<()> {
	let Some(channel_id) = settings.trimmed_channel_id() else {
		return fail(
			instance,
			"Join Voice Channel requires a configured Discord voice channel.",
		)
		.await;
	};

	match select_voice_channel(
		Some(channel_id),
		Some(settings.force_voice_move),
		Some(settings.navigate_to_channel),
	)
	.await
	{
		Ok(_) => {
			sync_join_voice_instance(instance, settings).await;
			instance.show_ok().await?;
			Ok(())
		}
		Err(error) => fail(instance, &error).await,
	}
}

async fn use_current_voice_channel(
	instance: &Instance,
	settings: &PluginActionSettings,
) -> OpenActionResult<()> {
	let Some(channel) = current_selected_voice_channel().await else {
		send_pi_status(
			instance,
			"Join a Discord voice or stage channel first, then try Use Current Voice Channel.",
			None,
		)
		.await;
		return Ok(());
	};

	let Some(guild_id) = channel.guild_id.clone() else {
		send_pi_status(
			instance,
			"Discord did not provide a guild for the current voice channel.",
			None,
		)
		.await;
		return Ok(());
	};

	let guild = match get_guild(guild_id.clone()).await {
		Ok(guild) => guild,
		Err(error) => {
			send_pi_status(
				instance,
				"Could not look up the current Discord server. The channel will still be saved without the server icon.",
				Some(&error),
			)
			.await;
			let mut next = settings.clone();
			next.guild_id = guild_id;
			next.channel_id = channel.id.clone();
			next.channel_name = channel
				.name
				.clone()
				.unwrap_or_else(|| "Current Voice Channel".to_owned());
			instance.set_settings(&next).await?;
			store_join_voice_instance(instance, &next).await;
			sync_join_voice_instance(instance, &next).await;
			return Ok(());
		}
	};

	let mut next = settings.clone();
	next.guild_id = guild.id;
	next.guild_name = guild.name;
	next.guild_icon_url = guild.icon_url;
	next.channel_id = channel.id.clone();
	next.channel_name = channel
		.name
		.clone()
		.unwrap_or_else(|| "Current Voice Channel".to_owned());
	instance.set_settings(&next).await?;
	store_join_voice_instance(instance, &next).await;
	sync_join_voice_instance(instance, &next).await;
	send_pi_status(instance, "Saved the current Discord voice channel to this button.", None).await;
	Ok(())
}

pub struct JoinVoiceChannelAction;
#[async_trait]
impl Action for JoinVoiceChannelAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.joinvoicechannel";
	type Settings = PluginActionSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		store_join_voice_instance(instance, settings).await;
		sync_join_voice_instance(instance, settings).await;
		Ok(())
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		join_voice_settings().write().await.remove(&instance.instance_id);
		join_voice_press_states()
			.write()
			.await
			.remove(&instance.instance_id);
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		store_join_voice_instance(instance, settings).await;
		sync_join_voice_instance(instance, settings).await;
		Ok(())
	}

	async fn property_inspector_did_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		seed_or_explain_join_voice_settings(instance, settings).await
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		payload: &Value,
	) -> OpenActionResult<()> {
		match payload.get("type").and_then(Value::as_str) {
			Some("voiceChannel/refresh") | Some("voiceChannel/useCurrent") => {
				use_current_voice_channel(instance, settings).await?;
			}
			_ => {}
		}
		Ok(())
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let Some(state_lock) = hold_state_for(&instance.instance_id).await else {
			return Ok(());
		};

		let hold_ms = settings.effective_hold_ms();
		let press_id = {
			let mut state = state_lock.lock().await;
			state.press_id += 1;
			state.handled_by_hold = false;
			state.press_id
		};

		let instance_id = instance.instance_id.clone();
		tokio::spawn(async move {
			sleep(Duration::from_millis(hold_ms)).await;

			let Some(state_lock) = hold_state_for(&instance_id).await else {
				return;
			};
			let should_leave = {
				let mut state = state_lock.lock().await;
				if state.press_id != press_id || state.handled_by_hold {
					false
				} else {
					state.handled_by_hold = true;
					true
				}
			};

			if !should_leave {
				return;
			}

			let instances = visible_instances(JoinVoiceChannelAction::UUID).await;
			let Some(instance) = instances
				.into_iter()
				.find(|candidate| candidate.instance_id == instance_id)
			else {
				return;
			};

			match leave_voice_channel().await {
				Ok(_) => {
					sync_join_voice_channel_states().await;
					if let Err(error) = instance.show_ok().await {
						log::warn!("Failed to show leave voice success: {}", error);
					}
				}
				Err(error) => {
					let _ = fail(&instance, &error).await;
				}
			}
		});

		Ok(())
	}

	async fn key_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let Some(state_lock) = hold_state_for(&instance.instance_id).await else {
			return Ok(());
		};

		let handled_by_hold = {
			let mut state = state_lock.lock().await;
			let handled = state.handled_by_hold;
			state.press_id += 1;
			state.handled_by_hold = false;
			handled
		};

		if handled_by_hold {
			return Ok(());
		}

		handle_join_voice_key_up(instance, settings).await
	}
}

pub struct LeaveVoiceChannelAction;
#[async_trait]
impl Action for LeaveVoiceChannelAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.leavevoicechannel";
	type Settings = PluginActionSettings;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		match leave_voice_channel().await {
			Ok(_) => {
				sync_join_voice_channel_states().await;
				Ok(())
			}
			Err(error) => fail(instance, &error).await,
		}
	}
}

pub struct SelectTextChannelAction;
#[async_trait]
impl Action for SelectTextChannelAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.selecttextchannel";
	type Settings = PluginActionSettings;

	async fn key_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let Some(channel_id) = settings.trimmed_channel_id() else {
			return fail(instance, "Select Text Channel requires a channel ID").await;
		};

		match select_text_channel(Some(channel_id)).await {
			Ok(_) => Ok(()),
			Err(error) => fail(instance, &error).await,
		}
	}
}
