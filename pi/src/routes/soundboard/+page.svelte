<script lang="ts">
	import { actionSettings, eventTarget } from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	interface SoundboardSound {
		name: string;
		guild_id: string;
		sound_id: string;
	}

	let sounds: SoundboardSound[] = $state([]);
	let search = $state("");
	let open = $state(false);

	let selectedSound = $derived($actionSettings.sound?.sound_id ?? "");

	let filteredSounds = $derived(
		sounds.filter((sound) =>
			sound.name.toLowerCase().includes(search.toLowerCase())
		)
	);

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.sounds)) {
			sounds = payload.sounds;

			if (
				(!selectedSound ||
					!sounds.some((s) => s.sound_id === selectedSound)) &&
				sounds.length > 0
			) {
				$actionSettings = {
					...$actionSettings,
					sound: sounds[0]
				};
			}
		}
	});

	function selectSound(sound: SoundboardSound) {
		$actionSettings = {
			...$actionSettings,
			sound
		};

		search = sound.name;
		open = false;
	}
</script>

// TODO: generated test page for debugging
<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-start">
		<label class="text-sm pt-2">Sound</label>

		<div class="relative">
			<input
				type="text"
				bind:value={search}
				placeholder="Search sounds..."
				class="w-full rounded border border-neutral-600 bg-neutral-800 px-3 py-2"
				onfocus={() => (open = true)}
			/>

			{#if open}
				<div
					class="absolute z-10 mt-1 max-h-64 w-full overflow-y-auto rounded border border-neutral-600 bg-neutral-800"
				>
					{#each filteredSounds as sound}
						<button
							type="button"
							class="block w-full px-3 py-2 text-left hover:bg-neutral-700"
							onclick={() => selectSound(sound)}
						>
							{sound.name}
						</button>
					{/each}

					{#if filteredSounds.length === 0}
						<div class="px-3 py-2 text-neutral-400">
							No sounds found
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>

<hr class="my-4 border-neutral-700" />

<ApplicationSettings />
