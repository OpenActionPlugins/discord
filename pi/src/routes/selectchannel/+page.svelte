<script lang="ts">
	import { actionSettings, eventTarget, sendToPlugin } from "@openaction/svelte-pi";
	import ApplicationSettings from "$lib/ApplicationSettings.svelte";
	import RefreshIcon from "$lib/assets/RefreshIcon.svelte";

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
	let loadingGuilds = $state(false);
	let loadingChannels = $state(false);

	let selectedGuild = $derived($actionSettings.guild_id ?? "");
	let selectedChannel = $derived($actionSettings.channel_id ?? "");

	function requestChannels(guild_id: string) {
		if (!guild_id) {
			channels = [];
			return;
		}
		loadingChannels = true;
		sendToPlugin({ action: "request_channels", guild_id });
	}

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.guilds)) {
			guilds = payload.guilds;
			loadingGuilds = false;
			if (selectedGuild && channels.length === 0) {
				requestChannels(selectedGuild);
			}
		}

		if (Array.isArray(payload.channels)) {
			channels = payload.channels;
			loadingChannels = false;
		}
	});

	function refreshGuilds() {
		guilds = [];
		channels = [];
		loadingGuilds = true;
		loadingChannels = true;
		sendToPlugin({ action: "refresh_guilds" });
	}

	function updateGuild(event: Event) {
	    channels = [];

		const guild_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, guild_id, channel_id: "" };
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
				disabled={loadingGuilds}
				title="Refresh servers"
				class="text-neutral-500 hover:text-neutral-200 disabled:opacity-30 transition-colors"
			>
				<RefreshIcon />
			</button>
		</div>
		<div class="select-wrapper">
			<select
				id="guild"
				value={selectedGuild}
				onchange={updateGuild}
				class="w-full"
				disabled={loadingGuilds}
			>
				{#if guilds.length === 0}
					<option value="" disabled>No servers available</option>
				{:else}
					<option value="">Select a server</option>
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
				disabled={loadingChannels || !selectedGuild}
			>
				{#if !selectedGuild}
					<option value="">Select a server first</option>
				{:else if channels.length === 0}
					<option value="" disabled>No channels available</option>
				{:else}
					<option value="">Select a channel</option>
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
