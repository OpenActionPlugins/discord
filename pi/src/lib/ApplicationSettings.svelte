<script lang="ts">
	import { globalSettings, openUrl } from "$lib/openaction";

	let editing = false;
	let clientId = "";
	let clientSecret = "";

	$: {
		if ($globalSettings.clientId != undefined) {
			clientId = $globalSettings.clientId;
		}
		if ($globalSettings.clientSecret != undefined) {
			clientSecret = $globalSettings.clientSecret;
		}
	}

	function handleSave() {
		$globalSettings = {
			...$globalSettings,
			clientId,
			clientSecret,
		};
		editing = false;
	}

	function handleCancel() {
		clientId = $globalSettings.clientId || "";
		clientSecret = $globalSettings.clientSecret || "";
		editing = false;
	}

	function maskSecret(secret: string): string {
		return secret ? "•".repeat(secret.length) : "";
	}
</script>

<h2 class="mb-3 text-sm font-semibold text-neutral-100">
	Discord Application Settings
</h2>

{#if $globalSettings.error}
	<div
		class="mb-3 rounded border border-red-700 bg-red-900/30 p-2 text-xs text-red-300"
	>
		<strong class="font-semibold">Error:</strong>
		{$globalSettings.error}
	</div>
{:else if $globalSettings.accessToken && !editing}
	<div
		class="mb-3 rounded border border-green-700 bg-green-900/30 p-2 text-xs text-green-300"
	>
		✓ Connected to Discord
	</div>
{/if}

<div class="mb-2 flex items-center gap-2">
	<span class="min-w-[90px] text-xs font-medium text-neutral-200">
		Client ID:
	</span>
	{#if editing}
		<input
			id="clientId"
			type="text"
			bind:value={clientId}
			placeholder="Enter client ID"
			class="flex-1 rounded border border-neutral-700 bg-neutral-800 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-600 focus:ring-1 focus:ring-neutral-600 focus:outline-none"
		/>
	{:else}
		<span class="text-xs text-neutral-400">{clientId || "Not set"}</span>
	{/if}
</div>

<div class="mb-3 flex items-center gap-2">
	<span class="min-w-[90px] text-xs font-medium text-neutral-200">
		Client Secret:
	</span>
	{#if editing}
		<input
			id="clientSecret"
			type="password"
			bind:value={clientSecret}
			placeholder="Enter client secret"
			class="flex-1 rounded border border-neutral-700 bg-neutral-800 px-2 py-1 text-xs text-neutral-100 placeholder-neutral-500 focus:border-neutral-600 focus:ring-1 focus:ring-neutral-600 focus:outline-none"
		/>
	{:else}
		<span class="text-xs text-neutral-400">
			{maskSecret(clientSecret) || "Not set"}
		</span>
	{/if}
</div>

{#if editing}
	<div class="mb-3 flex gap-2">
		<button
			on:click={handleSave}
			class="cursor-pointer rounded bg-neutral-700 px-3 py-1 text-xs text-white hover:bg-neutral-600"
		>
			Save
		</button>
		<button
			on:click={handleCancel}
			class="cursor-pointer rounded bg-neutral-800 px-3 py-1 text-xs text-neutral-300 hover:bg-neutral-700"
		>
			Cancel
		</button>
	</div>

	<div class="rounded bg-neutral-800 p-3 text-xs">
		<p class="mb-2 font-medium text-neutral-300">
			Setting up your Discord Application:
		</p>
		<ol class="ml-1 list-inside list-decimal space-y-1.5 text-neutral-400">
			<li>
				Visit the
				<button
					on:click={() =>
						openUrl("https://discord.com/developers/applications")}
					class="cursor-pointer text-blue-400 underline hover:text-blue-300"
				>
					Discord Developer Portal
				</button>
				and log in
			</li>
			<li>Click "New Application" in the top right and give it a name</li>
			<li>
				Once created, navigate to the "OAuth2" section in the left sidebar
			</li>
			<li>
				Under "Client information", you'll find your
				<strong class="text-neutral-300">Client ID</strong>
			</li>
			<li>
				Click "Reset Secret" to generate a new
				<strong class="text-neutral-300">Client Secret</strong>
			</li>
			<li>Scroll down to "Redirects" and click "Add Redirect"</li>
			<li>
				Enter any valid URL (e.g.,
				<code class="rounded bg-neutral-900 px-1 text-neutral-300">
					http://localhost
				</code>
				) - the value doesn't matter for this plugin
			</li>
			<li>Click "Save Changes" at the bottom</li>
			<li>Copy your Client ID and Client Secret into the fields above</li>
		</ol>
	</div>
{:else}
	<button
		on:click={() => (editing = true)}
		class="cursor-pointer rounded bg-neutral-700 px-3 py-1 text-xs text-white hover:bg-neutral-600"
	>
		Edit
	</button>
{/if}
