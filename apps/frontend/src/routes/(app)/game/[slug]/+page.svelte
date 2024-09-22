<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { client, websocketClient } from '$lib/client';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { Input } from '../../../../lib/components/ui/input';
	import type { LobbyData } from '@gangsta/rusty';
	import { accessToken } from '../../../../lib/stores/access-token';

	let lobby: LobbyData | undefined;

	// const response = await client.mutation(['lobby.subscribe', []]);
	// console.log(response);
	let unsubscribe: (() => void) | undefined;
	if ($accessToken) {
		if (unsubscribe) {
			unsubscribe();
		}
		unsubscribe = websocketClient.addSubscription(
			['lobby.subscribe', [$page.params.slug, $accessToken]],
			{
				onStarted() {
					console.log('started.');
				},
				onData(data) {
					console.log(data);
				},
				onError(e) {
					console.log('error when streaming');
					console.error(e);
				}
			}
		);
	}

	let value = '';

	async function submit() {
		const response = await client.mutation([
			'lobby.chat',
			{ text: value, lobby_id: $page.params.slug }
		]);
		console.log(response);
	}
</script>

hi

<form on:submit|preventDefault={submit}>
	<Input bind:value />
	<Button type="submit">chat</Button>
</form>
