<script lang="ts">
	import { onMount } from "svelte";
	import { get } from "svelte/store";

	import {
		actionSettings,
		eventTarget,
		globalSettings,
		openUrl,
		sendToPlugin,
	} from "@openaction/svelte-pi";

	type SettingsMap = Record<string, unknown>;

	const JOIN_VOICE_ACTION = "me.amankhanna.oadiscord.joinvoicechannel";
	const TEXT_CHANNEL_ACTION = "me.amankhanna.oadiscord.selecttextchannel";

	let actionId = "";
	let clientId = "";
	let clientSecret = "";
	let guildId = "";
	let guildName = "";
	let channelId = "";
	let channelName = "";
	let holdMs = 500;
	let forceVoiceMove = true;
	let navigateToChannel = true;
	let showUserCount = false;
	let showActiveState = true;
	let editingGlobal = false;

	let voicePickerStatus = "";
	let voicePickerWarning = "";

	function stringifySetting(value: unknown): string {
		return value == undefined ? "" : String(value);
	}

	function numberSetting(value: unknown, fallback: number): number {
		return typeof value === "number" && Number.isFinite(value)
			? value
			: Number.parseInt(String(value ?? ""), 10) || fallback;
	}

	function syncFromStores() {
		const globals = get(globalSettings) as SettingsMap;
		const action = get(actionSettings) as SettingsMap;

		clientId = stringifySetting(globals.clientId);
		clientSecret = stringifySetting(globals.clientSecret);
		guildId = stringifySetting(action.guildId);
		guildName = stringifySetting(action.guildName);
		channelId = stringifySetting(action.channelId);
		channelName = stringifySetting(action.channelName);
		holdMs = Math.max(250, numberSetting(action.holdMs, 500));
		forceVoiceMove =
			action.forceVoiceMove == undefined
				? true
				: Boolean(action.forceVoiceMove);
		navigateToChannel =
			action.navigateToChannel == undefined
				? true
				: Boolean(action.navigateToChannel);
		showUserCount =
			action.showUserCount == undefined
				? false
				: Boolean(action.showUserCount);
		showActiveState =
			action.showActiveState == undefined
				? true
				: Boolean(action.showActiveState);
	}

	function saveGlobalSettings() {
		$globalSettings = {
			...$globalSettings,
			clientId,
			clientSecret,
		};
		editingGlobal = false;
	}

	function updateActionSettings() {
		$actionSettings = {
			...$actionSettings,
			guildId,
			guildName,
			channelId,
			channelName,
			holdMs,
			forceVoiceMove,
			navigateToChannel,
			showUserCount,
			showActiveState,
		};
	}

	function updateTextChannelSettings() {
		$actionSettings = {
			...$actionSettings,
			guildId,
			channelId,
		};
	}

	function maskSecret(secret: string): string {
		return secret ? "•".repeat(secret.length) : "";
	}

	function handlePiMessage(event: Event) {
		const detail = (event as CustomEvent).detail as {
			payload?: {
				type?: string;
				status?: string;
				warning?: string | null;
			};
		};
		const payload = detail.payload;
		if (!payload?.type) {
			return;
		}

		if (payload.type === "voiceChannel/status") {
			voicePickerStatus = payload.status ?? "";
			voicePickerWarning = payload.warning ?? "";
		}
	}

	onMount(() => {
		syncFromStores();

		const unsubscribeGlobal = globalSettings.subscribe(syncFromStores);
		const unsubscribeAction = actionSettings.subscribe(syncFromStores);
		eventTarget.addEventListener("sendToPropertyInspector", handlePiMessage);

		void (async () => {
			const socketData = (window as Window & {
				connectOpenActionSocketData?: Promise<unknown[]>;
			}).connectOpenActionSocketData;
			if (socketData) {
				const args = await socketData;
				const rawActionInfo = args[4];
				if (typeof rawActionInfo === "string") {
					const actionInfo = JSON.parse(rawActionInfo);
					actionId = actionInfo.action || "";
				}
			}

			if (actionId === JOIN_VOICE_ACTION) {
				sendToPlugin({ type: "voiceChannel/useCurrent" });
			}
		})();

		return () => {
			unsubscribeGlobal();
			unsubscribeAction();
			eventTarget.removeEventListener("sendToPropertyInspector", handlePiMessage);
		};
	});

	$: needsJoinVoiceSettings = actionId === JOIN_VOICE_ACTION;
	$: needsTextChannelSettings = actionId === TEXT_CHANNEL_ACTION;
</script>

<h2 class="mb-3 text-sm font-semibold text-neutral-100">
	Discord Plugin Settings
</h2>

{#if $globalSettings.error}
	<div
		class="mb-3 rounded-lg border border-red-700 bg-red-900/30 p-2 text-xs text-red-300"
	>
		<strong class="font-semibold">Error:</strong>
		{$globalSettings.error}
	</div>
{:else if $globalSettings.accessToken}
	<div
		class="mb-3 rounded-lg border border-green-700 bg-green-900/30 p-2 text-xs text-green-300"
	>
		Connected to Discord RPC
	</div>
{/if}

<div class="mb-4 rounded-lg border border-neutral-700 bg-neutral-800/80 p-3">
	<div class="mb-3 flex items-center justify-between">
		<h3 class="text-xs font-semibold text-neutral-100">Global Authentication</h3>
		{#if !editingGlobal}
			<button
				on:click={() => (editingGlobal = true)}
				class="cursor-pointer rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-white hover:bg-neutral-600"
			>
				Edit
			</button>
		{/if}
	</div>

	<div class="mb-2 flex items-center gap-2">
		<span class="min-w-24 text-xs font-medium text-neutral-200">Client ID:</span>
		{#if editingGlobal}
			<input
				type="text"
				bind:value={clientId}
				placeholder="Discord OAuth client ID"
				class="flex-1 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-500 focus:ring-1 focus:ring-neutral-500 focus:outline-none"
			/>
		{:else}
			<span class="text-xs text-neutral-300">{clientId || "Not set"}</span>
		{/if}
	</div>

	<div class="mb-3 flex items-center gap-2">
		<span class="min-w-24 text-xs font-medium text-neutral-200">
			Client Secret:
		</span>
		{#if editingGlobal}
			<input
				type="password"
				bind:value={clientSecret}
				placeholder="Discord OAuth client secret"
				class="flex-1 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-500 focus:ring-1 focus:ring-neutral-500 focus:outline-none"
			/>
		{:else}
			<span class="text-xs text-neutral-300">
				{maskSecret(clientSecret) || "Not set"}
			</span>
		{/if}
	</div>

	{#if editingGlobal}
		<div class="flex gap-2">
			<button
				on:click={saveGlobalSettings}
				class="cursor-pointer rounded-lg border border-neutral-500 bg-neutral-600 px-3 py-1 text-xs text-white hover:bg-neutral-500"
			>
				Save
			</button>
			<button
				on:click={() => (editingGlobal = false)}
				class="cursor-pointer rounded-lg border border-neutral-600 bg-neutral-700 px-3 py-1 text-xs text-neutral-300 hover:bg-neutral-600"
			>
				Cancel
			</button>
		</div>
	{/if}
</div>

<div class="mb-4 rounded-lg border border-neutral-700 bg-neutral-800/80 p-3">
	<h3 class="mb-3 text-xs font-semibold text-neutral-100">Action Settings</h3>
	<p class="mb-3 text-xs text-neutral-400">
		Action:
		<span class="text-neutral-200">{actionId || "Loading..."}</span>
	</p>

	{#if needsJoinVoiceSettings}
		{#if voicePickerStatus}
			<div class="mb-3 rounded-lg border border-blue-700 bg-blue-900/20 p-2 text-xs text-blue-200">
				{voicePickerStatus}
			</div>
		{/if}

		{#if voicePickerWarning}
			<div class="mb-3 rounded-lg border border-amber-600 bg-amber-900/20 p-2 text-xs text-amber-200">
				{voicePickerWarning}
			</div>
		{/if}

		<div class="mb-3 flex gap-2">
			<button
				on:click={() => sendToPlugin({ type: "voiceChannel/useCurrent" })}
				class="cursor-pointer rounded-lg border border-neutral-500 bg-neutral-600 px-3 py-1 text-xs text-white hover:bg-neutral-500"
			>
				Use Current Voice Channel
			</button>
		</div>

		<div class="mb-3 rounded-lg border border-neutral-700 bg-neutral-900/50 p-2 text-xs text-neutral-400">
			The simplest setup is:
			<span class="text-neutral-200">join the voice channel you want in Discord, then click Use Current Voice Channel.</span>
		</div>

		<div class="mb-2 flex items-center gap-2">
			<span class="min-w-24 text-xs font-medium text-neutral-200">Manual Guild ID:</span>
			<input
				type="text"
				bind:value={guildId}
				on:change={updateActionSettings}
				placeholder="Optional manual guild ID"
				class="flex-1 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-500 focus:ring-1 focus:ring-neutral-500 focus:outline-none"
			/>
		</div>

		<div class="mb-3 flex items-center gap-2">
			<span class="min-w-24 text-xs font-medium text-neutral-200">Manual Channel ID:</span>
			<input
				type="text"
				bind:value={channelId}
				on:change={updateActionSettings}
				placeholder="Manual voice or stage channel ID"
				class="flex-1 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-500 focus:ring-1 focus:ring-neutral-500 focus:outline-none"
			/>
		</div>

		<div class="mb-2 flex items-center gap-2">
			<span class="min-w-24 text-xs font-medium text-neutral-200">Hold to Leave:</span>
			<input
				type="number"
				min="250"
				step="100"
				bind:value={holdMs}
				on:change={updateActionSettings}
				class="w-28 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 focus:border-neutral-500 focus:ring-1 focus:ring-neutral-500 focus:outline-none"
			/>
			<span class="text-xs text-neutral-400">ms</span>
		</div>

		<label class="mb-2 flex items-center gap-2 text-xs text-neutral-300">
			<input
				type="checkbox"
				bind:checked={forceVoiceMove}
				on:change={updateActionSettings}
			/>
			<span>Allow moving the user if they are already in voice</span>
		</label>
		<label class="mb-2 flex items-center gap-2 text-xs text-neutral-300">
			<input
				type="checkbox"
				bind:checked={navigateToChannel}
				on:change={updateActionSettings}
			/>
			<span>Navigate Discord to the channel after joining</span>
		</label>
		<label class="mb-2 flex items-center gap-2 text-xs text-neutral-300">
			<input
				type="checkbox"
				bind:checked={showUserCount}
				on:change={updateActionSettings}
			/>
			<span>Show how many people are in the current call on the button title</span>
		</label>
		<label class="flex items-center gap-2 text-xs text-neutral-300">
			<input
				type="checkbox"
				bind:checked={showActiveState}
				on:change={updateActionSettings}
			/>
			<span>Show connected state on the button</span>
		</label>

		<div class="mt-3 rounded-lg border border-neutral-700 bg-neutral-900/50 p-2 text-xs text-neutral-400">
			<div>Saved Server: <span class="text-neutral-200">{guildName || guildId || "Not set"}</span></div>
			<div>Saved Channel: <span class="text-neutral-200">{channelName || channelId || "Not set"}</span></div>
			<div class="mt-1 text-neutral-500">
				Tap the button to join this voice channel. Hold it for about {holdMs} ms to leave voice.
			</div>
		</div>
	{:else if needsTextChannelSettings}
		<div class="mb-2 flex items-center gap-2">
			<span class="min-w-24 text-xs font-medium text-neutral-200">Guild ID:</span>
			<input
				type="text"
				bind:value={guildId}
				on:change={updateTextChannelSettings}
				placeholder="Optional guild ID"
				class="flex-1 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-500 focus:ring-1 focus:ring-neutral-500 focus:outline-none"
			/>
		</div>

		<div class="mb-3 flex items-center gap-2">
			<span class="min-w-24 text-xs font-medium text-neutral-200">Channel ID:</span>
			<input
				type="text"
				bind:value={channelId}
				on:change={updateTextChannelSettings}
				placeholder="Required Discord channel ID"
				class="flex-1 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-500 focus:ring-1 focus:ring-neutral-500 focus:outline-none"
			/>
		</div>
	{:else}
		<p class="text-xs text-neutral-400">
			This action does not require extra per-button configuration.
		</p>
	{/if}
</div>

<div class="rounded-lg border border-neutral-700 bg-neutral-800/80 p-3 text-xs text-neutral-300">
	<p class="mb-2 font-medium text-neutral-200">Discord Developer Setup</p>
	<ol class="ml-1 list-inside list-decimal space-y-1.5">
		<li>
			Open the
			<button
				on:click={() => openUrl("https://discord.com/developers/applications")}
				class="cursor-pointer text-blue-400 underline hover:text-blue-300"
			>
				Discord Developer Portal
			</button>
			and create an application.
		</li>
		<li>Copy the Client ID and Client Secret into the Global Authentication section.</li>
		<li>Add at least one Redirect URI in the Discord OAuth2 settings.</li>
		<li>Authorize the app when Discord prompts you after saving the settings.</li>
		<li>Enable Discord Developer Mode if you need to manually copy a channel ID.</li>
	</ol>
</div>
