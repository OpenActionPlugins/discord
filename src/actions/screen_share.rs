use crate::client::discord_client;

use discord_ipc_rust::models::send::commands::SentCommand;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde_json::Value;

pub struct ToggleScreenshareAction;

#[async_trait]
impl Action for ToggleScreenshareAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglescreenshare";
	type Settings = Value;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let mut client_lock = discord_client().write().await;
		let Some(client) = client_lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		if let Err(e) = client.emit_command(&SentCommand::ToggleScreenshare).await {
			log::error!("Failed to toggle screenshare: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
