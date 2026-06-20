use crate::cache::notification_cache;
use crate::client::discord_client;

use discord_ipc_rust::models::send::commands::{SelectTextChannelArgs, SentCommand};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

pub async fn update_title(instance: &Instance) -> OpenActionResult<()> {
	let cache = notification_cache().read().await;
	let title = format!("{}", cache.len());

	if let Err(e) = instance.set_title(Some(title), None).await {
		log::error!("Failed to update notifications action title: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}

#[derive(Serialize, Deserialize, Default)]
pub enum NotificationsActionType {
	#[default]
	DoNothing,
	Clear,
	OpenAndClear,
	CycleRecentFirst,
	CycleOldestFirst,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NotificationsSettings {
	pub action_type: NotificationsActionType,
}

pub struct NotificationsAction;
#[async_trait]
impl Action for NotificationsAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.notifications";
	type Settings = NotificationsSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_title(instance).await
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let notification = match settings.action_type {
			NotificationsActionType::DoNothing => return Ok(()),
			NotificationsActionType::Clear => {
				notification_cache().write().await.clear();
				update_title(instance).await?;
				return Ok(());
			}
			NotificationsActionType::OpenAndClear => {
				let mut cache = notification_cache().write().await;
				let notification = cache.pop_back();
				cache.clear();
				notification
			}
			NotificationsActionType::CycleRecentFirst => {
				notification_cache().write().await.pop_back()
			}
			NotificationsActionType::CycleOldestFirst => {
				notification_cache().write().await.pop_front()
			}
		};

		let Some(notification) = notification else {
			instance.show_alert().await?;
			return Ok(());
		};

		update_title(instance).await?;

		let mut client_lock = discord_client().write().await;
		let Some(client) = client_lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		if let Err(e) = client
			.emit_command(&SentCommand::SelectTextChannel(SelectTextChannelArgs {
				channel_id: Some(notification.channel_id),
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
