use crate::client::get_discord_client;

use std::collections::HashMap;

use discord_ipc_rust::models::send::commands::{SentCommand, ToggleScreenshareArgs};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};

pub struct ToggleScreenshareAction;

#[async_trait]
impl Action for ToggleScreenshareAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglescreenshare";
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

			client
				.emit_command(&SentCommand::ToggleScreenshare(ToggleScreenshareArgs {
					pid: None,
				}))
				.await
		};

		if let Err(e) = result {
			log::error!("Failed to toggle screen share: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
