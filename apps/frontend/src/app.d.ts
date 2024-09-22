// See https://kit.svelte.dev/docs/types#app

import type { JWTPayload } from 'jose';

// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		interface Locals {
			accessToken?: string;
			user?: JWTPayload;
		}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}
	interface Window {
		__TAURI__: Record<string, unknown>?;
	}
}

export {};
