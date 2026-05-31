<script lang="ts">
	import { actionSettings, eventTarget, sendToPlugin } from "@openaction/svelte-pi";
	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	interface Guild {
		id: string;
		name: string;
	}

	interface Channel {
		id: string;
		name: string;
	}

	let guilds: Guild[] = $state([]);
	let channels: Channel[] = $state([]);
	let loading = $state(false);

	let selectedGuild = $derived($actionSettings.guild_id ?? "");
	let selectedChannel = $derived($actionSettings.channel_id ?? "");

	function requestChannels(guild_id: string) {
		if (!guild_id) {
			channels = [];
			return;
		}
		loading = true;
		sendToPlugin({ action: "request_channels", guild_id });
	}

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.guilds)) {
			guilds = payload.guilds;
			loading = false;
			if (selectedGuild && channels.length === 0) {
				requestChannels(selectedGuild);
			}
		}

		if (Array.isArray(payload.channels)) {
			channels = payload.channels;
			loading = false;
		}
	});

	function refreshGuilds() {
		guilds = [];
		channels = [];
		loading = true;
		sendToPlugin({ action: "refresh_guilds" });
	}

	function updateGuild(event: Event) {
		const guild_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, guild_id, channel_id: "" };
		channels = [];
		requestChannels(guild_id);
	}

	function updateChannel(event: Event) {
		const channel_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, channel_id };
	}
</script>

<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-center">
		<div class="flex items-center gap-1.5">
			<label for="guild" class="text-sm">Server</label>
			<button
				onclick={refreshGuilds}
				disabled={loading}
				title="Refresh servers"
				class="text-neutral-500 hover:text-neutral-200 disabled:opacity-30 transition-colors"
			>
				<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
					<path d="M21 3v5h-5" />
					<path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
					<path d="M8 16H3v5" />
				</svg>
			</button>
		</div>
		<div class="select-wrapper">
			<select
				id="guild"
				value={selectedGuild}
				onchange={updateGuild}
				class="w-full"
				disabled={loading}
			>
				{#if guilds.length === 0}
					<option value="" disabled>{loading ? "Loading..." : "No servers available"}</option>
				{:else}
					<option value="">-- Select a server --</option>
					{#each guilds as guild}
						<option value={guild.id}>{guild.name}</option>
					{/each}
				{/if}
			</select>
		</div>
	</div>

	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="channel" class="text-sm">Channel</label>
		<div class="select-wrapper">
			<select
				id="channel"
				value={selectedChannel}
				onchange={updateChannel}
				class="w-full"
				disabled={loading || !selectedGuild}
			>
				{#if !selectedGuild}
					<option value="">-- Select a server first --</option>
				{:else if channels.length === 0}
					<option value="" disabled>{loading ? "Loading..." : "No channels available"}</option>
				{:else}
					<option value="">-- Select a channel --</option>
					{#each channels as channel}
						<option value={channel.id}>{channel.name}</option>
					{/each}
				{/if}
			</select>
		</div>
	</div>
</div>

<hr class="my-4 border-neutral-700" />

<ApplicationSettings />
