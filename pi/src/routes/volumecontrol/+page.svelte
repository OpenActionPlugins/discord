<script lang="ts">
	import { actionSettings } from "@openaction/svelte-pi";
	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	type AudioDeviceType = "Input" | "Output";
	type StepDirection = "Increase" | "Decrease";

	const MIN_STEPS = 1;
	const MAX_STEPS = 10;
	const DEFAULT_STEPS = 2;
	const DEFAULT_AUDIO_DEVICE_TYPE: AudioDeviceType = "Output";
	const DEFAULT_STEP_DIRECTION: StepDirection = "Increase";

	let selectedAudioDeviceType: AudioDeviceType = $derived($actionSettings.device_type ?? DEFAULT_AUDIO_DEVICE_TYPE);
	let selectedStepDirection: StepDirection = $derived(
		$actionSettings.step_direction ?? DEFAULT_STEP_DIRECTION,
	);
	let currentSteps = $derived.by(() => {
		const value = Number($actionSettings.steps ?? DEFAULT_STEPS);
		if (!Number.isFinite(value)) return DEFAULT_STEPS;
		return Math.max(MIN_STEPS, Math.min(MAX_STEPS, Math.round(value)));
	});

	function updateAudioDeviceType(event: Event) {
		const value = (event.target as HTMLSelectElement).value as AudioDeviceType;
		$actionSettings = { ...$actionSettings, device_type: value };
	}

	function updateStepDirection(event: Event) {
		const value = (event.target as HTMLSelectElement).value as StepDirection;
		$actionSettings = { ...$actionSettings, step_direction: value };
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
					value={selectedAudioDeviceType}
					onchange={updateAudioDeviceType}
					class="w-full"
				>
					<option value="Input">Input</option>
					<option value="Output">Output</option>
				</select>
			</div>
		</div>
	</div>

	<div class="settings-grid">
		<label for="steps" class="pt-1 text-sm">Volume Step Direction</label>
		<div class="space-y-1">
			<div class="select-wrapper">
				<select
					id="stepDirection"
					value={selectedStepDirection}
					onchange={updateStepDirection}
					class="w-full"
				>
					<option value="Increase">Increase</option>
					<option value="Decrease">Decrease</option>
				</select>
			</div>
			<p class="text-xs text-neutral-400">
				Keypad only: choose to raise or lower volume.
			</p>
		</div>
	</div>

	<div class="settings-grid">
		<label for="steps" class="pt-1 text-sm">Volume Steps</label>
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
