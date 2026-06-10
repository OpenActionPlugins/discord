<script lang="ts">
	import {
		actionSettings,
		actionInfo,
		eventTarget,
	} from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	type UserVolumeControlActionType = "Increase" | "Decrease" | "Set" | "Mute";

	interface User {
		id: string;
		nick: string;
	}

	const MIN_STEP_SIZE = 1;
	const MAX_STEP_SIZE = 10;
	const MIN_SET_VOLUME = 0;
	const MAX_SET_VOLUME = 200;
	const DEFAULT_STEP_SIZE = 5;
	const DEFAULT_SET_VOLUME = 100;
	const DEFAULT_ACTION_TYPE: UserVolumeControlActionType = "Increase";

	let users: User[] = $state([]);

	let selectedUser = $derived($actionSettings.user_id ?? "");
	let selectedActionType: UserVolumeControlActionType = $derived(
		$actionSettings.action_type ?? DEFAULT_ACTION_TYPE,
	);
	let isSetVolume = $derived(selectedActionType === "Set");
	let isMute = $derived(selectedActionType === "Mute");

	let currentStepSize = $derived(
		$actionSettings.step_size ?? DEFAULT_STEP_SIZE,
	);
	let currentSetVolume = $derived(
		$actionSettings.set_volume ?? DEFAULT_SET_VOLUME,
	);

	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.users)) {
			users = payload.users;
			if (
				(!selectedUser || !users.some((c) => c.id === selectedUser)) &&
				users.length > 0
			) {
				$actionSettings = { ...$actionSettings, user_id: users[0].id };
			}
		}
	});

	function updateUser(event: Event) {
		const user_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, user_id };
	}

	function updateActionType(event: Event) {
		const action_type = (event.target as HTMLSelectElement)
			.value as UserVolumeControlActionType;
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
		<label for="user" class="text-sm">Target</label>
		<div class="select-wrapper">
			<select
				id="user"
				value={selectedUser}
				onchange={updateUser}
				class="w-full"
			>
				{#if users.length === 0}
					<option value="" disabled>No users found</option>
				{:else}
					{#each users as user}
						<option value={user.id}>{user.nick}</option>
					{/each}
				{/if}
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
					<option value="Mute">Mute</option>
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
					max={MAX_SET_VOLUME}
					value={currentSetVolume}
					oninput={updateSetVolume}
					class="h-1.5 w-full cursor-pointer"
				/>
				<span>{currentSetVolume}%</span>
			</div>
		</div>
	{:else if !isMute}
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
