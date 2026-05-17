<script lang="ts">
	import { actionSettings } from "@openaction/svelte-pi";
	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	type AudioType = "Input" | "Output";

	const MIN_STEPS = 1;
	const MAX_STEPS = 10;
	const DEFAULT_STEPS = 2;
	const DEFAULT_AUDIO_TYPE: AudioType = "Output";

	let selectedAudioType: AudioType = $derived($actionSettings.type ?? DEFAULT_AUDIO_TYPE);
	let currentSteps = $derived.by(() => {
		const value = Number($actionSettings.steps ?? DEFAULT_STEPS);
		if (!Number.isFinite(value)) return DEFAULT_STEPS;
		return Math.max(MIN_STEPS, Math.min(MAX_STEPS, Math.round(value)));
	});

	function updateAudioType(event: Event) {
		const value = (event.target as HTMLSelectElement).value as AudioType;
		$actionSettings = { ...$actionSettings, type: value };
	}

	function updateSteps(event: Event) {
		const value = Number((event.target as HTMLInputElement).value);
		if (!Number.isFinite(value)) return;

		const steps = Math.max(MIN_STEPS, Math.min(MAX_STEPS, Math.round(value)));
		$actionSettings = { ...$actionSettings, steps };
	}
</script>

<ApplicationSettings />

<div class="space-y-4 pt-1 text-xs text-neutral-200">
	<div class="settings-grid">
		<label for="audioType" class="pt-2 text-sm">Target</label>
		<div class="space-y-2">
			<div class="select-wrapper">
				<select
					id="audioType"
					value={selectedAudioType}
					onchange={updateAudioType}
					class="w-full"
				>
					<option value="Input">Input</option>
					<option value="Output">Output</option>
				</select>
			</div>
			<p class="text-xs text-neutral-400">
				Choose whether this control adjusts input or output volume.
			</p>
		</div>
	</div>

	<div class="settings-grid">
		<label for="steps" class="pt-1 text-sm">Volume Step</label>
		<div class="space-y-1">
			<div class="flex items-center justify-between text-xs">
				<span>{currentSteps}</span>
			</div>
			<input
				id="steps"
				type="range"
				min={MIN_STEPS}
				max={MAX_STEPS}
				step="1"
				value={currentSteps}
				oninput={updateSteps}
				class="h-1.5 w-full cursor-pointer accent-blue-500"
			/>
		</div>
	</div>
</div>
