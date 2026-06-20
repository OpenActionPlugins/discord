use discord_ipc_rust::models::send::commands::SelectTextChannelArgs;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

use crate::{actions::select_text_channel, cache::notification_cache};

async fn update_title(instance: &Instance) -> OpenActionResult<()> {
	let cache = notification_cache().read().await;
	let title = format!("{}", cache.len());

	if let Err(e) = instance.set_title(Some(title), None).await {
		log::error!("Failed to update notification action title: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}

#[derive(Serialize, Deserialize, Default)]
pub enum NotificationActionType {
	#[default]
	DoNothing,
	Clear,
	CycleRecentFirst,
	CycleOldestFirst,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NotificationSettings {
	pub action_type: NotificationActionType,
}

pub struct NotificationAction;

#[async_trait]
impl Action for NotificationAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.notification";
	type Settings = NotificationSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_title(instance).await
	}

	async fn did_receive_settings(
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
			NotificationActionType::DoNothing => return Ok(()),
			NotificationActionType::Clear => {
				notification_cache().write().await.clear();
				update_title(instance).await?;
				return Ok(());
			}
			NotificationActionType::CycleRecentFirst => {
				notification_cache().write().await.pop_back()
			}
			NotificationActionType::CycleOldestFirst => {
				notification_cache().write().await.pop_front()
			}
		};

		let Some(notification) = notification else {
			instance.show_alert().await?;
			return Ok(());
		};

		update_title(instance).await?;

		select_text_channel(
			instance,
			SelectTextChannelArgs {
				channel_id: Some(notification.channel_id),
				timeout: None,
			},
		)
		.await
	}
}
