import { decodeJwt } from 'jose';
import { readable, writable } from 'svelte/store';

export const accessToken = writable<string | undefined>(undefined);
export const user = readable<{ sub: string } | undefined>(undefined, (set) => {
	accessToken.subscribe((at) => {
		if (at) {
			const user = decodeJwt<{ sub: string }>(at);
			console.log(user);
			return set(user);
		}
		set(undefined);
	});
});
