<script lang="ts">
	import type { GameState } from '@gangsta/rusty';
	import { accessToken, user } from '../../stores/access-token';
	import Button from '../ui/button/button.svelte';
	import { client } from '../../client';

	export let game_state: GameState;
	export let join_code: string;

	$: self = game_state.players[$user?.sub || ''];

	async function ready() {
		await client.mutation(['lobby.ready', join_code]);
	}
</script>

<Button on:click={ready}>Ready up</Button>
{#if self.status === 'Spectator'}
	spectaty
{:else}
	looks like you're {self.status}
{/if}
<pre>{JSON.stringify(self)}</pre>
