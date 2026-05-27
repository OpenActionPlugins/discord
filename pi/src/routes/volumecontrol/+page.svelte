<script lang="ts">
	import { actionSettings, actionInfo } from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	type AudioDeviceType = "Input" | "Output";
	type VolumeControlActionType = "Increase" | "Decrease" | "Set";

	const MIN_STEP_SIZE = 1;
	const MAX_STEP_SIZE = 100;
	const MIN_SET_VOLUME = 0;
	const MAX_SET_VOLUME_INPUT = 100;
	const MAX_SET_VOLUME_OUTPUT = 200;
	const DEFAULT_STEP_SIZE = 5;
	const DEFAULT_SET_VOLUME = 100;
	const DEFAULT_AUDIO_DEVICE_TYPE: AudioDeviceType = "Input";
	const DEFAULT_ACTION_TYPE: VolumeControlActionType = "Increase";

	let selectedAudioDeviceType: AudioDeviceType = $derived(
		$actionSettings.device_type ?? DEFAULT_AUDIO_DEVICE_TYPE,
	);
	let selectedActionType: VolumeControlActionType = $derived(
		$actionSettings.action_type ?? DEFAULT_ACTION_TYPE,
	);
	let isSetVolume = $derived(selectedActionType === "Set");
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

	function updateActionType(event: Event) {
		const action_type = (event.target as HTMLSelectElement)
			.value as VolumeControlActionType;
		$actionSettings = { ...$actionSettings, action_type };
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

<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="audioDeviceType" class="text-sm">Target</label>
		<div class="select-wrapper">
			<select
				id="audioDeviceType"
				value={selectedAudioDeviceType}
				onchange={updateAudioDeviceType}
				class="w-full"
			>
				<option value="Input">Input</option>
				<option value="Output">Output</option>
			</select>
		</div>
	</div>

	{#if $actionInfo?.payload.controller !== "Encoder"}
		<div class="grid grid-cols-[250px_1fr] items-center">
			<label for="actionType" class="text-sm">Action</label>
			<div class="select-wrapper">
				<select
					id="actionType"
					value={selectedActionType}
					onchange={updateActionType}
					class="w-full"
				>
					<option value="Increase">Increase</option>
					<option value="Decrease">Decrease</option>
					<option value="Set">Set</option>
				</select>
			</div>
		</div>
	{/if}

	{#if isSetVolume}
		<div class="grid grid-cols-[250px_1fr] items-center">
			<label for="setVolume" class="text-sm">Volume Level</label>
			<div class="flex flex-row items-center space-x-4">
				<input
					id="setVolume"
					type="range"
					min={MIN_SET_VOLUME}
					max={maxSetVolume}
					value={currentSetVolume}
					oninput={updateSetVolume}
					class="h-1.5 w-full cursor-pointer"
				/>
				<span>{currentSetVolume}%</span>
			</div>
		</div>
	{:else}
		<div class="grid grid-cols-[250px_1fr] items-center">
			<label for="stepSize" class="text-sm">Volume Step Size</label>
			<div class="flex flex-row items-center space-x-4">
				<input
					id="stepSize"
					type="range"
					min={MIN_STEP_SIZE}
					max={MAX_STEP_SIZE}
					value={currentStepSize}
					oninput={updateStepSize}
					class="h-1.5 w-full cursor-pointer"
				/>
				<span>{currentStepSize}%</span>
			</div>
		</div>
	{/if}
</div>

<hr class="my-4 border-neutral-700" />

<ApplicationSettings />
