<script lang="ts">
	import {
		actionSettings,
		eventTarget,
		sendToPlugin,
	} from "@openaction/svelte-pi";

	import type { Guild } from "$lib/types";
	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

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

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.guilds)) {
			guilds = payload.guilds;
			loadingGuilds = false;
			if (
				(!selectedGuild || !guilds.some((g) => g.id === selectedGuild)) &&
				guilds.length > 0
			) {
				$actionSettings = { ...$actionSettings, guild_id: guilds[0].id };
			}

			loadingChannels = true;
			sendToPlugin({ action: "request_channels", guild_id: selectedGuild });
		}

		if (Array.isArray(payload.channels)) {
			channels = payload.channels;
			loadingChannels = false;
			if (
				(!selectedChannel || !channels.some((c) => c.id === selectedChannel)) &&
				channels.length > 0
			) {
				$actionSettings = { ...$actionSettings, channel_id: channels[0].id };
			}
		}
	});

	function updateGuild(event: Event) {
		const guild_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, guild_id, channel_id: "" };

		channels = [];
		loadingChannels = true;
		sendToPlugin({ action: "request_channels", guild_id: selectedGuild });
	}

	function updateChannel(event: Event) {
		const channel_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, channel_id };
	}
</script>

<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="guild" class="text-sm">Server</label>
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
					<option value="" disabled>Select a server</option>
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
					<option value="" disabled>Select a server first</option>
				{:else if channels.length === 0}
					<option value="" disabled>No channels available</option>
				{:else}
					<option value="" disabled>Select a channel</option>
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
