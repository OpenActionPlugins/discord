use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PluginActionSettings {
	#[serde(rename = "guildId")]
	pub guild_id: String,
	#[serde(rename = "guildName")]
	pub guild_name: String,
	#[serde(rename = "guildIconUrl")]
	pub guild_icon_url: String,
	#[serde(rename = "channelId")]
	pub channel_id: String,
	#[serde(rename = "channelName")]
	pub channel_name: String,
	#[serde(rename = "holdMs")]
	pub hold_ms: u64,
	#[serde(rename = "forceVoiceMove")]
	pub force_voice_move: bool,
	#[serde(rename = "navigateToChannel")]
	pub navigate_to_channel: bool,
	#[serde(rename = "showUserCount")]
	pub show_user_count: bool,
	#[serde(rename = "showActiveState")]
	pub show_active_state: bool,
}

impl Default for PluginActionSettings {
	fn default() -> Self {
		Self {
			guild_id: String::new(),
			guild_name: String::new(),
			guild_icon_url: String::new(),
			channel_id: String::new(),
			channel_name: String::new(),
			hold_ms: 500,
			force_voice_move: true,
			navigate_to_channel: true,
			show_user_count: true,
			show_active_state: true,
		}
	}
}

impl PluginActionSettings {
	#[allow(dead_code)]
	pub fn trimmed_guild_id(&self) -> Option<String> {
		let value = self.guild_id.trim();
		if value.is_empty() {
			None
		} else {
			Some(value.to_owned())
		}
	}

	pub fn trimmed_channel_id(&self) -> Option<String> {
		let value = self.channel_id.trim();
		if value.is_empty() {
			None
		} else {
			Some(value.to_owned())
		}
	}

	pub fn trimmed_channel_name(&self) -> Option<String> {
		let value = self.channel_name.trim();
		if value.is_empty() {
			None
		} else {
			Some(value.to_owned())
		}
	}

	pub fn trimmed_guild_icon_url(&self) -> Option<String> {
		let value = self.guild_icon_url.trim();
		if value.is_empty() {
			None
		} else {
			Some(value.to_owned())
		}
	}

	pub fn effective_hold_ms(&self) -> u64 {
		self.hold_ms.max(250)
	}
}
