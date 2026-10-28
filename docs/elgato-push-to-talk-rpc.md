# Elgato Stream Deck Discord plugin: undocumented `PUSH_TO_TALK` RPC

Named-pipe MITM of Discord IPC on Windows while using Elgato Stream Deck Pedal + official Discord plugin (`ESDDiscord`, app id `1224385725949284403`).

Public Discord RPC documents `SET_VOICE_SETTINGS` (`mute`, `mode.type`, `mode.shortcut`) and has **no** “PTT is currently held” command. The Elgato plugin still produces a solid speaking indicator for the whole pedal hold, even with Discord in Push to Talk and an empty shortcut. That is not Voice Activity (which only lights the ring while audio crosses the threshold).

## Capture method

Proxy is server on `\\.\pipe\discord-ipc-0` before Discord starts. Discord binds `ipc-1`. Stream Deck connects to `0`. Frames are `u32le opcode`, `u32le length`, JSON. Opcode `0` handshake, `1` FRAME.

Low-level keyboard/mouse hooks during holds saw **no** injected keys.

## Connect-time voice state

`GET_VOICE_SETTINGS` / `VOICE_SETTINGS_UPDATE`:

```json
"mode": {
  "type": "PUSH_TO_TALK",
  "auto_threshold": true,
  "threshold": -60,
  "shortcut": [],
  "delay": 20
}
```

Empty shortcut stays empty after press. Elgato does not plant a hidden `mode.shortcut`.

## Pedal hold (plugin → Discord)

Every down:

```json
{"cmd": "SET_VOICE_SETTINGS", "args": {"mute": false}}
{"cmd": "PUSH_TO_TALK", "args": {"active": true}}
```

Every up:

```json
{"cmd": "SET_VOICE_SETTINGS", "args": {"mute": false}}
{"cmd": "PUSH_TO_TALK", "args": {"active": false}}
```

Discord replies `PUSH_TO_TALK` with `"data": null`, then `SPEAKING_START` / `SPEAKING_STOP`. That is the green ring for the whole hold.

`SET_VOICE_SETTINGS` on press only sends `mute: false` (including on release). The gate is `PUSH_TO_TALK.active`.

## What this is not

- Not `PTT_DOWN` / `PTT_UP`
- Not `SendInput` / overlay key injection
- Not toggling `mode.type` to `VOICE_ACTIVITY` for the hold
- Not a keyboard shortcut you bind in Discord settings

## Linux

This is copyable RPC. After normal `AUTHORIZE` / `AUTHENTICATE` with `rpc.voice.write`, send the same two frames. If the Linux Discord client implements `PUSH_TO_TALK`, the pedal can stay in Discord PTT with no shortcut. If it does not, Discord will return an RPC error instead of `SPEAKING_START`.

OpenAction plugin `PushToTalkAction` previously only sent `SET_VOICE_SETTINGS { mute: false/true }`, which is VAD-style unmute. Matching Elgato means sending `PUSH_TO_TALK { active }` as well.

## Scopes Elgato uses

`rpc`, `identify`, `rpc.voice.read`, `rpc.voice.write`, plus video/screenshare/notifications.
