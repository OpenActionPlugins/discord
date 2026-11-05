use super::audio_device_utils::{AudioDeviceType, AudioDeviceWrapper, get_audio_device_settings};
use super::update_voice_setting;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait, visible_instances};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Default)]
pub enum VolumeControlActionType {
	#[default]
	Increase,
	Decrease,
	Set,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct VolumeControlSettings {
	pub device_type: AudioDeviceType,
	pub action_type: VolumeControlActionType,
	pub step_size: u8,
	pub set_volume: u8,
}

impl Default for VolumeControlSettings {
	fn default() -> Self {
		Self {
			device_type: AudioDeviceType::Input,
			action_type: VolumeControlActionType::default(),
			step_size: 5,
			set_volume: 100,
		}
	}
}

async fn adjust_volume(
	instance: &Instance,
	device_type: &AudioDeviceType,
	value: f32,
	set: bool,
) -> OpenActionResult<()> {
	let Some(device_settings) = get_audio_device_settings(device_type).await else {
		log::error!(
			"Failed to obtain voice settings for {:?} device",
			device_type
		);
		instance.show_alert().await?;
		return Ok(());
	};

	let current_linear = device_type.to_linear(device_settings.volume);
	let new_linear =
		if set { value } else { current_linear + value }.clamp(0.0, device_type.max_volume());

	if new_linear == current_linear {
		return Ok(());
	}

	let updated_settings = AudioDeviceWrapper {
		volume: device_type.to_discord(new_linear),
		..device_settings
	};

	update_voice_setting(instance, updated_settings.into(), 0).await
}

const BAR_COLOR: &str = "#5865F2"; // Discord "blurple"
const BAR_BOOST_COLOR: &str = "#ED4245"; // Discord "red", used above 100%

// White line-art glyph for the device type, drawn inside a 144x144 canvas (upper area).
fn device_glyph(device_type: &AudioDeviceType) -> &'static str {
	match device_type {
		AudioDeviceType::Input => {
			r##"<g fill="none" stroke="#fff" stroke-width="6" stroke-linecap="round"><rect x="60" y="12" width="24" height="40" rx="12" fill="#fff" stroke="none"/><path d="M48 38a24 24 0 0 0 48 0"/><path d="M72 62v8M62 70h20"/></g>"##
		}
		AudioDeviceType::Output => {
			r##"<g fill="none" stroke="#fff" stroke-width="6" stroke-linecap="round"><path d="M44 54v-6a28 28 0 0 1 56 0v6"/><rect x="38" y="48" width="16" height="24" rx="6" fill="#fff" stroke="none"/><rect x="90" y="48" width="16" height="24" rx="6" fill="#fff" stroke="none"/></g>"##
		}
	}
}

fn svg_data_uri(svg: &str) -> String {
	format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg))
}

// Key image: device glyph, the volume percentage and a rounded volume bar, on a transparent
// background. For output devices (which go up to 200%) a small tick marks the 100% point and
// the bar turns red past it.
fn key_image(device_type: &AudioDeviceType, percent: i64) -> String {
	const BAR_X: f32 = 16.0;
	const BAR_W: f32 = 112.0;

	let max = device_type.max_volume();
	let fraction = (percent as f32 / max).clamp(0.0, 1.0);
	let fill_w = BAR_W * fraction;
	let color = if percent > 100 {
		BAR_BOOST_COLOR
	} else {
		BAR_COLOR
	};

	let fill = if fill_w > 0.0 {
		format!(
			r##"<rect x="{BAR_X}" y="114" width="{fill_w:.1}" height="12" rx="6" fill="{color}"/>"##
		)
	} else {
		String::new()
	};
	let tick = if max > 100.0 {
		let x = BAR_X + BAR_W * (100.0 / max);
		format!(
			r##"<rect x="{:.1}" y="110" width="2" height="20" rx="1" fill="#fff" fill-opacity="0.6"/>"##,
			x - 1.0
		)
	} else {
		String::new()
	};

	let svg = format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">{glyph}<text x="72" y="102" text-anchor="middle" font-family="sans-serif" font-weight="700" font-size="28" fill="#fff">{percent}%</text><rect x="{BAR_X}" y="114" width="{BAR_W}" height="12" rx="6" fill="#fff" fill-opacity="0.2"/>{fill}{tick}</svg>"##,
		glyph = device_glyph(device_type),
	);
	svg_data_uri(&svg)
}

// Dial icon for the "$B1" layout: just the device glyph.
fn dial_icon(device_type: &AudioDeviceType) -> String {
	svg_data_uri(&format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144"><g transform="translate(0 30)">{}</g></svg>"##,
		device_glyph(device_type)
	))
}

// Renders the current device volume onto the key (custom image) or the dial (native "$B1"
// layout: icon + title + value + bar). The layout is declared in the manifest, not re-sent here,
// to avoid the flicker described in `user_volume_control::update_display`.
async fn update_display(
	instance: &Instance,
	settings: &VolumeControlSettings,
) -> OpenActionResult<()> {
	let device_type = &settings.device_type;
	let Some(device_settings) = get_audio_device_settings(device_type).await else {
		return Ok(());
	};
	let percent = device_type.to_linear(device_settings.volume).round() as i64;

	if instance.controller == "Encoder" {
		let title = match device_type {
			AudioDeviceType::Input => "Input",
			AudioDeviceType::Output => "Output",
		};
		let indicator = ((percent as f32 / device_type.max_volume()) * 100.0).round() as i64;
		let color = if percent > 100 {
			BAR_BOOST_COLOR
		} else {
			BAR_COLOR
		};
		let feedback = json!({
			"icon": dial_icon(device_type),
			"title": title,
			"value": format!("{percent}%"),
			"indicator": { "value": indicator.clamp(0, 100), "bar_fill_c": color },
		});
		if let Err(e) = instance.set_feedback(&feedback).await {
			log::error!("Failed to set feedback for volume control: {}", e);
		}
	} else if let Err(e) = instance
		.set_image(Some(key_image(device_type, percent)), None)
		.await
	{
		log::error!("Failed to set image for volume control: {}", e);
	}

	Ok(())
}

// Re-reads settings on every visible Volume Control instance so their displays refresh
// (round-trips through `did_receive_settings`, which knows each instance's device type).
pub async fn refresh_instances() {
	for instance in visible_instances(VolumeControlAction::UUID).await {
		let _ = instance.get_settings().await;
	}
}

pub struct VolumeControlAction;
#[async_trait]
impl Action for VolumeControlAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.volumecontrol";
	type Settings = VolumeControlSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_display(instance, settings).await
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_display(instance, settings).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let value = match settings.action_type {
			VolumeControlActionType::Increase => settings.step_size as f32,
			VolumeControlActionType::Decrease => -(settings.step_size as f32),
			VolumeControlActionType::Set => settings.set_volume as f32,
		};

		adjust_volume(
			instance,
			&settings.device_type,
			value,
			matches!(settings.action_type, VolumeControlActionType::Set),
		)
		.await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let delta = (settings.step_size as f32) * ticks as f32;

		adjust_volume(instance, &settings.device_type, delta, false).await
	}
}
