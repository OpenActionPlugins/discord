use discord_ipc_rust::models::send::commands::SelectTextChannelArgs;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

use crate::{actions::select_text_channel, cache::notification_cache};

async fn update_title(instance: &Instance) -> OpenActionResult<()> {
	let cache = notification_cache().read().await;
	let title = format!("{}", cache.len());

	instance.set_title(Some(title), None).await
}

#[derive(Serialize, Deserialize, Default)]
pub enum NotificationActionType {
	#[default]
	Show,
	Clear,
	CycleRecentFirst,
	CycleRecentLast,
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
		let mut cache = notification_cache().write().await;

		if matches!(settings.action_type, NotificationActionType::Clear) {
			cache.clear();
			return Ok(());
		}

		let notification = match settings.action_type {
			NotificationActionType::Clear => unreachable!(),
			NotificationActionType::Show | NotificationActionType::CycleRecentFirst => {
				cache.pop_front()
			}
			NotificationActionType::CycleRecentLast => cache.pop_back(),
		};

		let Some(notification) = notification else {
			instance.show_alert().await?;
			return Ok(());
		};

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
