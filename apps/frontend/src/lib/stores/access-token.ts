import { writable } from 'svelte/store';

export const accessToken = writable<string | undefined>(undefined);
