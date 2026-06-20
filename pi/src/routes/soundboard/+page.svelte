<script lang="ts">
	import { actionSettings, eventTarget } from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	interface Guild {
		id: string;
		name: string;
	}

	interface SoundboardSound {
		name: string;
		guild_id: string;
		sound_id: string;
		emoji_id: string | null;
		emoji_name: string | null;
	}

	let guildMap = $state(new Map<string, string>());
	let sounds: SoundboardSound[] = $state([]);

	let search = $state("");
	let selectedSound = $derived($actionSettings.sound?.sound_id ?? "");

	let groupedSounds = $derived.by(() => {
		const groups = new Map<
			string,
			{ label: string; sounds: SoundboardSound[] }
		>();

		for (const sound of sounds) {
			if (!sound.name.toLowerCase().includes(search.toLowerCase())) continue;

			if (!groups.has(sound.guild_id)) {
				groups.set(sound.guild_id, {
					label:
						guildMap.get(sound.guild_id) ??
						(sound.guild_id === "0" ? "Discord sounds" : sound.guild_id),
					sounds: [],
				});
			}

			groups.get(sound.guild_id)!.sounds.push(sound);
		}

		return [...groups.values()].sort((a, b) =>
			a.label === "Built-in"
				? -1
				: b.label === "Built-in"
					? 1
					: a.label.localeCompare(b.label),
		);
	});

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.guilds)) {
			guildMap = new Map(payload.guilds.map((g: Guild) => [g.id, g.name]));
		}

		if (Array.isArray(payload.sounds)) {
			sounds = payload.sounds;
		}
	});

	function selectSound(sound: SoundboardSound) {
		$actionSettings = { ...$actionSettings, sound };
	}
</script>

<div class="space-y-3 text-neutral-200">
	<input
		type="text"
		bind:value={search}
		placeholder="Search sounds..."
		class="w-full rounded-lg border border-neutral-600 bg-neutral-700 px-3 py-2 text-sm text-neutral-300 outline-hidden"
	/>

	<div class="space-y-3">
		{#each groupedSounds as group}
			<div>
				<div class="mb-2 text-center tracking-wide text-white">
					{group.label}
				</div>
				<div class="grid grid-cols-2 gap-1">
					{#each group.sounds as sound}
						<button
							type="button"
							onclick={() => selectSound(sound)}
							class="flex items-center gap-2 truncate rounded-lg border px-3 py-2 text-sm transition-colors
								{sound.sound_id === selectedSound
								? 'border-blue-500 bg-blue-600 text-white'
								: 'border-neutral-600 bg-neutral-700 text-neutral-300 hover:bg-neutral-600'}"
						>
							{#if sound.emoji_id}
								<img
									src="https://cdn.discordapp.com/emojis/{sound.emoji_id}.png"
									alt=""
									class="h-4 w-4 shrink-0"
								/>
							{:else if sound.emoji_name}
								<span class="text-sm">{sound.emoji_name}</span>
							{/if}

							<span class="truncate">{sound.name}</span>
						</button>
					{/each}
				</div>
			</div>
		{/each}

		{#if groupedSounds.length === 0}
			<div class="py-4 text-center text-sm text-neutral-500">
				No sounds found
			</div>
		{/if}
	</div>
</div>

<hr class="my-4 border-neutral-700" />

<ApplicationSettings />
