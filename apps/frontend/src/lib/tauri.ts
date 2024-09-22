import { invoke, isTauri as forTauri } from '@tauri-apps/api/core';
import { client } from './client';
import { browser } from '$app/environment';
import { type UnlistenFn } from '@tauri-apps/api/event';

export const isTauri = browser && forTauri();

export async function getRefreshTokenFromTauri() {
	return (await invoke('get_refresh_token')) as string | null;
}

export async function saveRefreshTokenTauri(token: string) {
	const response = await invoke('set_refresh_token', { token });

	return response;
}

export async function getAccessTokenWithTauri() {
	const refreshToken = await getRefreshTokenFromTauri();
	if (refreshToken) {
		const response = await client.mutation(['authentication.refresh_token', refreshToken]);
		return response.access_token;
	}
}

export async function scanForOtpQrs(details: unknown): Promise<string[]> {
	return await invoke('scan_qr', { details });
}

export async function prepForQrCodeScan() {
	return await invoke('prep_qr');
}

export async function openQrScanner() {
	return await invoke('start_qr');
}

export function createTauriListeners() {
	if (!isTauri) {
		return () => {};
	}
	console.log('setup listeners.');

	const unsubscribers: UnlistenFn[] = [];
	// listen<string[]>('qr_results', async (results) => {
	// 	// const accountDetails = await client.query(['account.preview', results.payload]);
	// 	// accountDetails.map(createAccount);
	// }).then((unsub) => unsubscribers.push(unsub));

	return () => {
		for (const unsub of unsubscribers) {
			unsub();
		}
	};
}
