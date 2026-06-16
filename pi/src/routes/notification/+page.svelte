<script lang="ts">
	import { actionSettings } from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	let selectedActionType = $derived($actionSettings.action_type ?? "Show");

	function updateActionType(event: Event) {
		const action_type = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, action_type };
	}
</script>

<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="deviceTarget" class="text-sm">Action Type</label>
		<div class="select-wrapper">
			<select
				id="deviceTarget"
				value={selectedActionType}
				onchange={updateActionType}
				class="w-full"
			>
				<option value="Show">Show</option>
				<option value="Clear">Clear</option>
				<option value="CycleRecentFirst">Cycle, recent first</option>
				<option value="CycleRecentLast">Cycle, recent last</option>
			</select>
		</div>
	</div>
</div>

<hr class="my-4 border-neutral-700" />

<ApplicationSettings />
