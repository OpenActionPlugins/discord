use crate::client::get_discord_client;

use std::collections::HashMap;

use discord_ipc_rust::models::send::commands::SentCommand;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};

pub struct ToggleVideoAction;

#[async_trait]
impl Action for ToggleVideoAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglevideo";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let result = {
			let Some(mut client) = get_discord_client(instance).await? else {
				return Ok(());
			};

			client.emit_command(&SentCommand::ToggleVideo).await
		};

		if let Err(e) = result {
			log::error!("Failed to toggle video: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
