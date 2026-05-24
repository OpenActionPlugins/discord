<script lang="ts">
	import { actionSettings, eventTarget } from "@openaction/svelte-pi";
	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	interface AudioDevice {
		id: string;
		name: string;
	}

	type AudioDeviceType = "Input" | "Output" | "Both";

	const DEFAULT_DEVICE_TYPE: AudioDeviceType = "Output";

	let inputs: AudioDevice[] = $state([]);
	let outputs: AudioDevice[] = $state([]);

	let selectedDeviceType = $derived(
		($actionSettings.type as AudioDeviceType | undefined) ?? DEFAULT_DEVICE_TYPE,
	);
	let selectedInput = $derived($actionSettings.input_device_id ?? "");
	let selectedOutput = $derived($actionSettings.output_device_id ?? "");

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.input_devices)) {
			inputs = payload.input_devices;
		}
		if (Array.isArray(payload.output_devices)) {
			outputs = payload.output_devices;
		}
	});

	function updateDeviceType(event: Event) {
		const value = (event.target as HTMLSelectElement).value as AudioDeviceType;
		$actionSettings = { ...$actionSettings, type: value };
	}

	function updateInputDevice(event: Event) {
		const value = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, input_device_id: value };
	}

	function updateOutputDevice(event: Event) {
		const value = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, output_device_id: value };
	}
</script>

<div class="space-y-4 pt-1 text-xs text-neutral-200">
	<div class="settings-grid">
		<label for="audioType" class="pt-2 text-sm">Target</label>
		<div class="space-y-2">
			<div class="select-wrapper">
				<select
					id="audioType"
					value={selectedDeviceType}
					onchange={updateDeviceType}
					class="w-full"
				>
					<option value="Input">Set Input</option>
					<option value="Output">Set Output</option>
					<option value="Both">Set Both</option>
				</select>
			</div>
		</div>
	</div>

	{#if selectedDeviceType === "Input" || selectedDeviceType === "Both"}
		<div class="settings-grid">
			<label for="inputDevice" class="pt-1 text-sm">Input device</label>
			<div class="space-y-1">
				<div class="select-wrapper">
					<select
						id="inputDevice"
						value={selectedInput}
						onchange={updateInputDevice}
						class="w-full"
					>
						{#if inputs.length === 0}
							<option value="" disabled selected>No input devices available</option>
						{:else}
							{#each inputs as device}
								<option value={device.id}>{device.name}</option>
							{/each}
						{/if}
					</select>
				</div>
			</div>
		</div>
	{/if}

	{#if selectedDeviceType === "Output" || selectedDeviceType === "Both"}
		<div class="settings-grid">
			<label for="outputDevice" class="pt-1 text-sm">Output device</label>
			<div class="space-y-1">
				<div class="select-wrapper">
					<select
						id="outputDevice"
						value={selectedOutput}
						onchange={updateOutputDevice}
						class="w-full"
					>
						{#if outputs.length === 0}
							<option value="" disabled selected>No output devices available</option>
						{:else}
							{#each outputs as device}
								<option value={device.id}>{device.name}</option>
							{/each}
						{/if}
					</select>
				</div>
			</div>
		</div>
	{/if}
</div>

<ApplicationSettings />
