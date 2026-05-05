## OpenAction Discord Plugin

An open-source Discord plugin for OpenDeck/OpenAction-based hosts. This project uses documented Discord local RPC commands where available and keeps Linux/OpenDeck compatibility as a first-class goal.

This repo is intentionally independent from Elgato's proprietary Discord plugin. It does not ship Elgato binaries, icons, localization files, or copied plugin assets.

## Credits

- Original OpenAction/OpenDeck Discord plugin base: `nekename`
- Feature direction, testing, and expanded plugin functionality in this fork: `StrikorDawn`
  - Implementation assistance during the current upgrade work: `OpenAI Codex`

### Runtime and Structure

- Backend: Rust
- Property inspector: Svelte 5 + Vite
- Host API: OpenAction / OpenDeck-compatible property inspector socket API

Key project areas:

- [src/main.rs](./src/main.rs): plugin startup, global settings, action registration
- [src/client.rs](./src/client.rs): Discord IPC connection, OAuth, request/response helpers
- [src/rpc_events.rs](./src/rpc_events.rs): Discord event handling and state syncing
- [src/actions](./src/actions): action implementations and per-button settings
- [assets/manifest.json](./assets/manifest.json): action catalog for OpenDeck
- [pi](./pi): property inspector UI

## Features

Implemented actions:

- Toggle Mute
- Toggle Deafen
- Push to Talk
- Push to Mute
- Toggle Voice Mode
- Join Voice Channel
- Leave Voice Channel
- Select Text Channel

Documented Discord RPC features used:

- `GET_GUILDS`
- `GET_CHANNELS`
- `GET_VOICE_SETTINGS`
- `SET_VOICE_SETTINGS`
- `SELECT_VOICE_CHANNEL`
- `SELECT_TEXT_CHANNEL`
- `GET_SELECTED_VOICE_CHANNEL`

## Setup

### 1. Create a Discord developer application

1. Open <https://discord.com/developers/applications>
2. Create a new application
3. Open the `OAuth2` section
4. Copy the `Client ID`
5. Generate or copy the `Client Secret`
6. Add at least one Redirect URI

Current OAuth scopes used by this plugin:

- `rpc`
- `identify`
- `rpc.voice.read`
- `rpc.voice.write`

### 2. Configure the plugin in OpenDeck

1. Install/build the plugin bundle
2. Open the property inspector for any Discord action
3. Enter the global `Client ID` and `Client Secret`
4. Save and approve the Discord authorization prompt
5. For `Join Voice Channel`, join the target Discord voice/stage channel in Discord first, then use `Use Current Voice Channel` in the property inspector
6. Tap the button to join that voice channel, or hold it for about 1 second to leave voice
7. For `Select Text Channel`, enter the target `Channel ID`
## Linux / OpenDeck Notes

- The plugin targets Discord local IPC, so the Discord desktop client must be running locally.
- On Linux, Discord IPC usually resolves through `XDG_RUNTIME_DIR`, `TMPDIR`, `TMP`, `TEMP`, or `/tmp`.
- If OpenDeck starts before Discord, reconnect may take a few seconds after Discord launches.

### Nobara / Linux Troubleshooting

- Confirm the Discord desktop app is running, not only the browser client.
- If actions alert immediately, restart Discord and then reopen OpenDeck.
- If OAuth repeatedly fails, recheck the Client ID, Client Secret, and that the Discord app has a Redirect URI configured.
- If IPC is still unavailable, test with the standard Discord build before Flatpak/Snap-specific debugging.

## Known Limitations

- `Join Voice Channel` now prefers `Use Current Voice Channel` plus manual ID fallback, while `Select Text Channel` still uses a manual channel ID field.
- `Leave Voice Channel` depends on Discord accepting `SELECT_VOICE_CHANNEL` with a null channel id. If a Discord build rejects that behavior, leaving voice will remain limited by the client.
- `Toggle Voice Mode` only works when Discord returns a normal `VOICE_ACTIVITY` or `PUSH_TO_TALK` mode payload.
- This plugin only controls the local desktop client. It does not work with browser-only Discord sessions.
- Discord RPC must already be authenticated and available for current-channel detection to work.

## Build

Prerequisites:

- Rust toolchain with `cargo`
- Node.js and npm

Build the property inspector:

```bash
cd pi
npm install
npm run build
```

Build the Rust plugin binary:

```bash
cargo build --release
```

Bundle with the helper script:

```bash
./build.sh <output_directory> <binary_name> <target_triple>
```

Example:

```bash
./build.sh ~/.config/opendeck/plugins/me.amankhanna.oadiscord.sdPlugin oadiscord x86_64-unknown-linux-gnu
```

## Development Checks

Suggested commands:

```bash
cargo fmt
cargo build
cd pi && npm run build
cd pi && npm run check
```

## Error Handling

The plugin surfaces and logs common operational failures, including:

- Discord not running
- Discord IPC unavailable
- OAuth/token exchange failures
- Missing Client ID or Client Secret
- Missing Channel ID for channel actions
- RPC command timeouts or unsupported command errors

## Manual Testing Still Recommended

After building, verify these directly against a running Discord desktop client:

- Existing mute/deafen/PTT actions still behave correctly
- Voice mode toggles between `VOICE_ACTIVITY` and `PUSH_TO_TALK`
- `Use Current Voice Channel` saves the current Discord voice channel even if live guild enumeration is unavailable
- A short press on Join Voice Channel joins the configured channel
- A long press on Join Voice Channel leaves the current voice channel without rejoining
- The Join Voice Channel button can show the configured guild icon while idle and an optional live in-call count in the title
- Join/leave/select channel actions behave correctly in your Discord build
- Error messages are clear when Discord is closed or credentials are invalid
