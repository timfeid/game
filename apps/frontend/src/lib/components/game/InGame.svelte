<script lang="ts">
	import type { GameState } from '@gangsta/rusty';
	import { accessToken, user } from '../../stores/access-token';
	import Button from '../ui/button/button.svelte';
	import { client } from '../../client';
	import CCard from './Card.svelte';
	import { toast } from 'svelte-sonner';
	import { RSPCError } from '@rspc/client';
	import Player from './Player.svelte';
	import { writable } from 'svelte/store';
	import type { EffectTarget } from './type';
	import { convertEffectTarget, waitForTarget } from './game';

	export let game_state: GameState;
	export let join_code: string;

	$: self = game_state.players[$user?.sub || ''];

	$: isMyTurn = self.player_index === game_state.public_info.current_turn?.current_player_index;

	async function turn() {
		await client.mutation(['lobby.turn', join_code]);
	}

	async function executePlayCard(index: number, target?: EffectTarget) {
		try {
			await client.mutation([
				'lobby.play_card',
				{
					code: join_code,
					in_hand_index: index,
					target: target ? convertEffectTarget(target) : null
				}
			]);
		} catch (e) {
			if (e instanceof RSPCError) {
				return toast.error(e.message);
			}
			toast.error('Unknown error!');
		}
	}
	async function playCard(index: number) {
		const card = self.hand[index];
		const target = await waitForTarget(card, game_state, true);
		return await executePlayCard(index, target);
	}
</script>

{#if isMyTurn}
	<div>it's my turn {game_state.public_info.current_turn?.phase}</div>
{/if}

{#if isMyTurn}
	<Button on:click={turn}>Turn</Button>
{/if}

<div class=" fixed bottom-0 left-0 right-0 bg-gray-950 border-t">
	<div class="container mx-auto py-2">
		<div class="grid gap-2 grid-cols-7">
			{#each self.hand as card, i}
				<CCard on:click={() => playCard(i)} cardWithDetails={card}></CCard>
			{/each}
		</div>
	</div>
</div>

<div class="fixed font-mono top-0 right-0 bg-gray-950 border-b border-l px-4 py-2">
	<div class="uppercase border-b mb-1 pb-1">Mana pool</div>
	<div>
		{#each Object.keys(self.public_info.mana_pool) as key}
			{@const val = self.public_info.mana_pool[key]}
			<div>
				{key}: {val}
			</div>
		{/each}
	</div>
</div>

<div class="space-y-4">
	{#each Object.keys(game_state.players) as key}
		{@const player = game_state.players[key]}
		<Player code={join_code} game={game_state} {player} playerName={key} />
	{/each}
</div>

<pre>{JSON.stringify(self)}</pre>
