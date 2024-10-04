<script lang="ts">
	import { browser } from '$app/environment';
	import { page } from '$app/stores';
	import { client, websocketClient } from '$lib/client';
	import { accessToken } from '$lib/stores/access-token';
	import type { LobbyData } from '@gangsta/rusty';
	import { Loader } from 'lucide-svelte';
	import { type ComponentType } from 'svelte';
	import { toast } from 'svelte-sonner';
	import InGame from './InGame.svelte';
	import Lobby from './Lobby.svelte';

	let lobby: LobbyData | undefined;
	let unsubscribe: (() => void) | undefined;

	if (browser && $accessToken) {
		reset($accessToken);
	}

	async function reset(accessToken: string) {
		if (unsubscribe) {
			unsubscribe();
		}
		unsubscribe = websocketClient.addSubscription(
			['lobby.subscribe', [$page.params.slug, accessToken]],
			{
				onStarted() {
					console.log('started.');
				},
				onData(data) {
					console.log(data);
					lobby = data;
				},
				onError(e) {
					console.log('error when streaming');
					console.error(e);
				}
			}
		);

		await join();
	}
	export let code: string;

	async function join() {
		try {
			await client.mutation(['lobby.join', $page.params.slug]);
		} catch (e) {
			console.error(e);
			toast.error('Something went wrong');
		}
	}

	let component: ComponentType;
	$: {
		if (!lobby) {
			component = Loader;
		} else if (
			lobby?.game_state.status === 'NeedsPlayers' ||
			typeof lobby.game_state.status !== 'string'
		) {
			component = Lobby;
		} else {
			component = InGame;
		}
	}
</script>

{#if lobby}
	<svelte:component this={component} {...lobby} />
	{#if $page.url.searchParams.has('debug')}
		<pre class="mt-16">{JSON.stringify(lobby, null, 2)}</pre>
	{/if}
{/if}
