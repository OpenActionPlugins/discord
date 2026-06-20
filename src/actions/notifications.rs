use crate::cache::NOTIFICATION_CACHE;
use crate::client::get_discord_client;

use discord_ipc_rust::models::send::commands::{SelectTextChannelArgs, SentCommand};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

pub async fn update_title(instance: &Instance) -> OpenActionResult<()> {
	let cache = NOTIFICATION_CACHE.read().await;
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
				NOTIFICATION_CACHE.write().await.clear();
				update_title(instance).await?;
				return Ok(());
			}
			NotificationsActionType::OpenAndClear => {
				let mut cache = NOTIFICATION_CACHE.write().await;
				let notification = cache.pop_back();
				cache.clear();
				notification
			}
			NotificationsActionType::CycleRecentFirst => {
				NOTIFICATION_CACHE.write().await.pop_back()
			}
			NotificationsActionType::CycleOldestFirst => {
				NOTIFICATION_CACHE.write().await.pop_front()
			}
		};

		let Some(notification) = notification else {
			instance.show_alert().await?;
			return Ok(());
		};

		update_title(instance).await?;

		let result = {
			let Some(mut client) = get_discord_client(instance).await? else {
				return Ok(());
			};
			client
				.emit_command(&SentCommand::SelectTextChannel(SelectTextChannelArgs {
					channel_id: Some(notification.channel_id),
					timeout: None,
				}))
				.await
		};

		if let Err(e) = result {
			log::error!("Failed to select text channel: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
