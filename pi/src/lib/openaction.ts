import { writable, get } from "svelte/store";

export const globalSettings = writable<any>({});
export const actionSettings = writable<any>({});

export const eventTarget = new EventTarget();

let ws: WebSocket, action: string, context: string;

export function sendToPlugin(payload: any) {
	if (ws?.readyState == WebSocket.OPEN) {
		ws.send(
			JSON.stringify({
				event: "sendToPlugin",
				action,
				context,
				payload,
			}),
		);
	}
}

export function openUrl(url: string) {
	if (ws?.readyState == WebSocket.OPEN) {
		ws.send(
			JSON.stringify({
				event: "openUrl",
				payload: { url },
			}),
		);
	}
}

// @ts-expect-error
if (globalThis.connectOpenActionSocketData) {
	const [port, propertyInspectorUUID, registerEvent, _info, actionInfo] =
		// @ts-expect-error
		await globalThis.connectOpenActionSocketData;
	ws = new WebSocket("ws://localhost:" + port);

	const actionData = JSON.parse(actionInfo);
	action = actionData.action;
	context = actionData.context;

	let actionSettingsSubscribed = false,
		globalSettingsSubscribed = false;
	actionSettings.set(actionData.payload.settings ?? {});
	actionSettings.subscribe((settings) => {
		if (!actionSettingsSubscribed) {
			actionSettingsSubscribed = true;
			return;
		}
		ws.send(
			JSON.stringify({
				event: "setSettings",
				context,
				payload: settings,
			}),
		);
	});

	ws.onopen = () => {
		ws.send(
			JSON.stringify({
				event: registerEvent,
				uuid: propertyInspectorUUID,
			}),
		);

		ws.send(
			JSON.stringify({
				event: "getGlobalSettings",
				context,
			}),
		);
	};

	ws.onmessage = (event) => {
		const json = JSON.parse(event.data);
		if (json.event == "didReceiveSettings") {
			const settings = json.payload.settings;
			if (settings != get(actionSettings)) actionSettings.set(settings);
		} else if (json.event == "didReceiveGlobalSettings") {
			const settings = json.payload.settings;
			if (settings != get(globalSettings)) globalSettings.set(settings);

			globalSettings.subscribe((settings) => {
				if (!globalSettingsSubscribed) {
					globalSettingsSubscribed = true;
					return;
				}
				ws.send(
					JSON.stringify({
						event: "setGlobalSettings",
						context,
						payload: settings,
					}),
				);
			});
		} else {
			eventTarget.dispatchEvent(new CustomEvent(json.event, { detail: json }));
		}
	};
}
