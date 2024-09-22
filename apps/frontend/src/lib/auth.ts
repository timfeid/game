import type { AuthResponse } from '@gangsta/rusty';
import { redirect } from '@sveltejs/kit';
import { accessToken } from './stores/access-token';
import { isTauri, getAccessTokenWithTauri, saveRefreshTokenTauri } from './tauri';

export function loginRequired({ accessToken }: { accessToken?: string }) {
	if (!accessToken) {
		throw redirect(307, '/login');
	}
}

export async function refreshAccessToken() {
	const token = (await getAccessToken()) || undefined;
	accessToken.set(token);

	return token;
}

export async function getAccessToken() {
	if (isTauri) {
		return getAccessTokenWithTauri();
	}

	const response = await fetch('/refresh-token', { method: 'post' });
	const token = await response.text();

	return token;
}

export async function saveLoginDetails(details: AuthResponse) {
	accessToken.set(details.access_token || undefined);
	if (isTauri && details.refresh_token) {
		return saveRefreshTokenTauri(details.refresh_token);
		// return console.log(await invoke('save_login', { refreshToken: details.refresh_token }));
	}

	await fetch(`/save-login`, {
		method: 'post',
		body: JSON.stringify(details),
		headers: {
			'Content-Type': 'application/json'
		}
	});
}
