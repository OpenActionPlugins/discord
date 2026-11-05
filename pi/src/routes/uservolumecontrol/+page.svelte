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
		avatar: string;
	}

	const MIN_STEP_SIZE = 1;
	const MAX_STEP_SIZE = 100;
	const MIN_SET_VOLUME = 0;
	const MAX_SET_VOLUME = 200;
	const DEFAULT_STEP_SIZE = 5;
	const DEFAULT_SET_VOLUME = 100;
	const DEFAULT_ACTION_TYPE: UserVolumeControlActionType = "Increase";

	let users: User[] = $state([]);
	let userDropdownOpen = $state(false);

	let selectedUser = $derived($actionSettings.user_id ?? "");
	let selectedUserObj = $derived(users.find((user) => user.id === selectedUser));
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

	function selectUser(user: User) {
		$actionSettings = { ...$actionSettings, user_id: user.id };
		userDropdownOpen = false;
	}

	function handleWindowClick(event: MouseEvent) {
		if (
			userDropdownOpen &&
			!(event.target as HTMLElement).closest(".user-dropdown")
		) {
			userDropdownOpen = false;
		}
	}

	function handleAvatarError(event: Event) {
		(event.target as HTMLElement).remove();
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

<svelte:window onclick={handleWindowClick} />

<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="user" class="text-sm">Target</label>
		<div class="user-dropdown relative">
			<button
				id="user"
				type="button"
				class="flex w-full cursor-pointer items-center gap-2 rounded-lg border border-neutral-600 bg-neutral-700 px-3 py-2 text-left text-sm text-neutral-300 outline-hidden"
				onclick={() => (userDropdownOpen = !userDropdownOpen)}
				disabled={users.length === 0}
			>
				{#if selectedUserObj}
					<span
						class="h-5 w-5 shrink-0 overflow-hidden rounded-full bg-neutral-600"
					>
						<img
							src={selectedUserObj.avatar}
							alt=""
							class="h-full w-full object-cover"
							onerror={handleAvatarError}
						/>
					</span>
					<span class="truncate">{selectedUserObj.nick}</span>
				{:else}
					<span>No users found</span>
				{/if}
				<span
					class="ml-auto h-2 w-2 shrink-0 rotate-45 border-r-2 border-b-2 border-neutral-400"
				></span>
			</button>

			{#if userDropdownOpen && users.length > 0}
				<ul
					class="absolute z-10 mt-1 max-h-48 w-full overflow-y-auto rounded-lg border border-neutral-600 bg-neutral-700 shadow-lg"
				>
					{#each users as user}
						<li>
							<button
								type="button"
								class="flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left text-sm text-neutral-300 hover:bg-neutral-600"
								onclick={() => selectUser(user)}
							>
								<span
									class="h-5 w-5 shrink-0 overflow-hidden rounded-full bg-neutral-600"
								>
									<img
										src={user.avatar}
										alt=""
										class="h-full w-full object-cover"
										onerror={handleAvatarError}
									/>
								</span>
								<span class="truncate">{user.nick}</span>
							</button>
						</li>
					{/each}
				</ul>
			{/if}
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
