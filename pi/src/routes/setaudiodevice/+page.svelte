<script lang="ts">
	import { actionSettings, eventTarget } from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	interface VoiceAvailableDevice {
		id: string;
		name: string;
	}

	type AudioDeviceTarget = "Input" | "Output" | "Both";

	const DEFAULT_DEVICE_TARGET: AudioDeviceTarget = "Input";

	let inputs: VoiceAvailableDevice[] = $state([]);
	let outputs: VoiceAvailableDevice[] = $state([]);

	let selectedDeviceTarget: AudioDeviceTarget = $derived(
		$actionSettings.target ?? DEFAULT_DEVICE_TARGET,
	);
	let currentlySelectedInputDevice = $state("");
	let currentlySelectedOutputDevice = $state("");
	let selectedInputDevice = $derived($actionSettings.input_device_id ?? currentlySelectedInputDevice);
	let selectedOutputDevice = $derived($actionSettings.output_device_id ?? currentlySelectedOutputDevice);

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.input_devices)) {
			inputs = payload.input_devices;
		}
		if (Array.isArray(payload.output_devices)) {
			outputs = payload.output_devices;
		}
		if (typeof payload.selected_input_device === "string") {
			currentlySelectedInputDevice = payload.selected_input_device;
		}
		if (typeof payload.selected_output_device === "string") {
			currentlySelectedOutputDevice = payload.selected_output_device;
		}
	});

	function updateDeviceTarget(event: Event) {
		const target = (event.target as HTMLSelectElement).value as AudioDeviceTarget;
		$actionSettings = { ...$actionSettings, target };
	}

	function updateInputDevice(event: Event) {
		const input_device_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, input_device_id };
	}

	function updateOutputDevice(event: Event) {
		const output_device_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, output_device_id };
	}
</script>

<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="deviceTarget" class="text-sm">Target</label>
		<div class="select-wrapper">
			<select
				id="deviceTarget"
				value={selectedDeviceTarget}
				onchange={updateDeviceTarget}
				class="w-full"
			>
				<option value="Input">Set Input</option>
				<option value="Output">Set Output</option>
				<option value="Both">Set Both</option>
			</select>
		</div>
	</div>

	{#if selectedDeviceTarget === "Input" || selectedDeviceTarget === "Both"}
		<div class="grid grid-cols-[250px_1fr] items-center">
			<label for="inputDevice" class="text-sm">Input Device</label>
			<div class="select-wrapper">
				<select
					id="inputDevice"
					value={selectedInputDevice}
					onchange={updateInputDevice}
					class="w-full"
				>
					{#if inputs.length === 0}
						<option value="" disabled>No input devices available</option>
					{:else}
						{#each inputs as device}
							<option value={device.id}>{device.name}</option>
						{/each}
					{/if}
				</select>
			</div>
		</div>
	{/if}

	{#if selectedDeviceTarget === "Output" || selectedDeviceTarget === "Both"}
		<div class="grid grid-cols-[250px_1fr] items-center">
			<label for="outputDevice" class="text-sm">Output Device</label>
			<div class="select-wrapper">
				<select
					id="outputDevice"
					value={selectedOutputDevice}
					onchange={updateOutputDevice}
					class="w-full"
				>
					{#if outputs.length === 0}
						<option value="" disabled>No output devices available</option>
					{:else}
						{#each outputs as device}
							<option value={device.id}>{device.name}</option>
						{/each}
					{/if}
				</select>
			</div>
		</div>
	{/if}
</div>

<hr class="my-4 border-neutral-700" />

<ApplicationSettings />
