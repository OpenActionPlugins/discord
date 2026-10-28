use crate::cache::notification_cache;
use crate::client::discord_client;

use discord_ipc_rust::models::send::commands::{SelectTextChannelArgs, SentCommand};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

const NOTIFICATION_SVG: &str = include_str!("../../assets/actions/notifications.svg");

fn badge_xml(count: usize) -> String {
	if count == 0 {
		return String::new();
	}
	let label = if count > 999 {
		"999+".to_string()
	} else {
		count.to_string()
	};
	let width = (20 + label.len() * 9).max(36);
	let x = 114 - width / 2;

	format!(
		r#"<rect x="{x}" y="96" width="{width}" height="36" rx="18" fill="red"/>
        <text x="114" y="120" font-size="20" font-weight="bold" fill="white" text-anchor="middle" font-family="Arial, sans-serif">
            {label}
        </text>
        "#
	)
}

pub async fn update_image(instance: &Instance) -> OpenActionResult<()> {
	use base64::{Engine, prelude::BASE64_STANDARD};

	let count = notification_cache().read().await.len();
	let svg = NOTIFICATION_SVG.replace("</svg>", &format!("{}\n</svg>", badge_xml(count)));
	let data_url = format!("data:image/svg+xml;base64,{}", BASE64_STANDARD.encode(svg));

	if let Err(e) = instance.set_image(Some(data_url), None).await {
		log::error!("Failed to update notifications action image: {}", e);
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
		update_image(instance).await
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
				update_image(instance).await?;
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

		update_image(instance).await?;

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
