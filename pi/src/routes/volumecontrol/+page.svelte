<script lang="ts">
	import { actionSettings } from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	type AudioDeviceType = "Input" | "Output";
	type KeypadActionType = "Increase" | "Decrease" | "Set";

	const MIN_STEP_SIZE = 1;
	const MAX_STEP_SIZE = 10;
	const MIN_SET_VOLUME = 0;
	const MAX_SET_VOLUME_INPUT = 100;
	const MAX_SET_VOLUME_OUTPUT = 200;
	const DEFAULT_STEP_SIZE = 2;
	const DEFAULT_SET_VOLUME = 80;
	const DEFAULT_AUDIO_DEVICE_TYPE: AudioDeviceType = "Input";
	const DEFAULT_KEYPAD_ACTION_TYPE: KeypadActionType = "Increase";

	let selectedAudioDeviceType: AudioDeviceType = $derived(
		$actionSettings.device_type ?? DEFAULT_AUDIO_DEVICE_TYPE,
	);
	let selectedKeypadActionType: KeypadActionType = $derived(
		$actionSettings.keypad_action_type ?? DEFAULT_KEYPAD_ACTION_TYPE,
	);
	let isSetVolume = $derived(selectedKeypadActionType === "Set");
	let maxSetVolume = $derived(
		selectedAudioDeviceType === "Output"
			? MAX_SET_VOLUME_OUTPUT
			: MAX_SET_VOLUME_INPUT,
	);

	let currentStepSize = $derived(
		$actionSettings.step_size ?? DEFAULT_STEP_SIZE,
	);
	let currentSetVolume = $derived(
		$actionSettings.set_volume ?? DEFAULT_SET_VOLUME,
	);

	function updateAudioDeviceType(event: Event) {
		const device_type = (event.target as HTMLSelectElement)
			.value as AudioDeviceType;
		$actionSettings = { ...$actionSettings, device_type };
		if (currentSetVolume > maxSetVolume) {
			$actionSettings = { ...$actionSettings, set_volume: maxSetVolume };
		}
	}

	function updateKeypadActionType(event: Event) {
		const keypad_action_type = (event.target as HTMLSelectElement)
			.value as KeypadActionType;
		$actionSettings = { ...$actionSettings, keypad_action_type };
	}

	function updateStepSize(event: Event) {
		const step_size = parseInt((event.target as HTMLInputElement).value);
		$actionSettings = { ...$actionSettings, step_size };
	}

	function updateSetVolume(event: Event) {
		const set_volume = parseInt((event.target as HTMLInputElement).value);
		$actionSettings = { ...$actionSettings, set_volume };
	}
</script>

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
		<label for="keypadActionType" class="pt-1 text-sm">Keypad Action</label>
		<div class="space-y-1">
			<div class="select-wrapper">
				<select
					id="keypadActionType"
					value={selectedKeypadActionType}
					onchange={updateKeypadActionType}
					class="w-full"
				>
					<option value="Increase">Increase Volume</option>
					<option value="Decrease">Decrease Volume</option>
					<option value="Set">Set Volume</option>
				</select>
			</div>
			<p class="settings-description">
				Keypad only!! Does not affect dial action on press.
			</p>
		</div>
	</div>

	{#if isSetVolume}
		<div class="settings-grid">
			<label for="setVolume" class="pt-1 text-sm">Set Volume Level</label>
			<div class="space-y-1">
				<div class="flex items-center justify-between text-xs">
					<span>{currentSetVolume}%</span>
				</div>
				<input
					id="setVolume"
					type="range"
					min={MIN_SET_VOLUME}
					max={maxSetVolume}
					step="1"
					value={currentSetVolume}
					oninput={updateSetVolume}
					class="h-1.5 w-full cursor-pointer accent-blue-500"
				/>
				<p class="settings-description">Volume set by keypad press.</p>
			</div>
		</div>
	{:else}
		<div class="settings-grid">
			<label for="stepSize" class="pt-1 text-sm">Volume Step Size</label>
			<div class="space-y-1">
				<div class="flex items-center justify-between text-xs">
					<span>{currentStepSize}%</span>
				</div>
				<input
					id="stepSize"
					type="range"
					min={MIN_STEP_SIZE}
					max={MAX_STEP_SIZE}
					step="1"
					value={currentStepSize}
					oninput={updateStepSize}
					class="h-1.5 w-full cursor-pointer accent-blue-500"
				/>
				<p class="settings-description">Volume adjustment per step.</p>
			</div>
		</div>
	{/if}
</div>

<ApplicationSettings />
