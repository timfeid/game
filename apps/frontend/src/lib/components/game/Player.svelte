<script lang="ts">
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import type { CardWithDetails, FrontendTarget, GameState, PlayerState } from '@gangsta/rusty';
	import { RSPCError } from '@rspc/client';
	import { RectangleVertical } from 'lucide-svelte';
	import HeartPulse from 'lucide-svelte/icons/heart-pulse';
	import { toast } from 'svelte-sonner';
	import { client } from '../../client';
	import { user } from '../../stores/access-token';
	import CCard from './Card.svelte';
	import { searchingForTarget, target, waitForTarget } from './game';

	export let game: GameState;
	export let code: string;
	export let playerName: string;
	export let player: PlayerState;

	$: self = game.players[$user?.sub || ''];

	async function executeAction(index: number, card: CardWithDetails, target?: FrontendTarget) {
		try {
			await client.mutation([
				card.action_type === 'Attach' ? 'lobby.attach_card' : 'lobby.action_card',
				{
					code,
					player_index: player.player_index,
					in_play_index: index,
					target: target || null
				}
			]);
		} catch (e) {
			if (e instanceof RSPCError) {
				return toast.error(e.message);
			}
			toast.error('Unknown error!');
		}
	}

	async function actionSpell(index: number) {
		console.log(index, 'was clicked on', playerName, player);
		console.log(self);
		if ($searchingForTarget) {
			console.log('set target.');
			target.set({ Card: { player_index: player.player_index, card_index: index, pile: 'Spell' } });
			return;
		}
		const card = player.public_info.cards_in_play[index];
		if (self.player_index === player.player_index) {
			const target = await waitForTarget(card, game);
			return await executeAction(index, card, target);
		}
	}

	async function actionCard(index: number) {
		console.log(index, 'was clicked on', playerName, player);
		console.log(self);
		if ($searchingForTarget) {
			console.log('set target.');
			target.set({ Card: { player_index: player.player_index, card_index: index, pile: 'Play' } });
			return;
		}
		const card = player.public_info.cards_in_play[index];
		if (self.player_index === player.player_index) {
			try {
				const target = await waitForTarget(card, game);
				await executeAction(index, card, target);
			} catch (e) {
				toast.error((e as Error).toString());
			}
		}
	}

	async function setPlayerTarget(player: PlayerState) {
		target.set({ Player: player.player_index });
	}
</script>

<Card class="dark:bg-gray-950">
	<CardHeader class="space-y-1">
		<CardTitle class="text-2xl font-bold text-center flex items-center">
			<button on:click={() => setPlayerTarget(player)}>
				<div class="mr-4 flex items-center space-x-1">
					<div>
						{playerName}
					</div>
					<sub class="text-xs">
						{#if playerName === $user.sub}(it's you){/if}
					</sub>
				</div>
			</button>
			<div class="flex space-x-2 items-center"></div>
			<div class="ml-auto flex flex-col">
				<div class="flex items-center">
					{#each { length: player.public_info.hand_size } as _}
						<RectangleVertical />
					{/each}
					<HeartPulse />
					<div class="ml-2">
						{player.public_info.health}
					</div>
				</div>
				{#if Object.values(player.public_info.mana_pool).find((x) => x !== true && x > 0)}
					<div class="ml-auto flex space-x-2">
						<div class="text-xs uppercase">Mana pool</div>
						{#each Object.keys(player.public_info.mana_pool) as key}
							{@const val = player.public_info.mana_pool[key]}
							{#if Number.isInteger(val)}
								{#each { length: val } as _, i}
									<div
										class="w-4 h-4 rounded-full"
										class:bg-gray-100={key === 'colorless'}
										class:bg-green-400={key === 'green'}
										class:bg-blue-400={key === 'blue'}
									></div>
								{/each}
							{/if}
						{/each}
					</div>
				{/if}
			</div>
		</CardTitle>
	</CardHeader>
	<CardContent class="space-y-4">
		<div class="flex flex-wrap gap-2">
			{#each player.public_info.cards_in_play as card, i}
				{#if typeof card.card.card_type !== 'string'}
					<CCard on:click={() => actionCard(i)} cardWithDetails={card}></CCard>
				{/if}
			{/each}
		</div>
		<div class="flex flex-wrap gap-2">
			{#each player.public_info.cards_in_play as card, i}
				{#if typeof card.card.card_type === 'string'}
					<CCard on:click={() => actionCard(i)} cardWithDetails={card}></CCard>
				{/if}
			{/each}
			{#each player.public_info.spells as card, i}
				<CCard class="opacity-50" on:click={() => actionSpell(i)} cardWithDetails={card}></CCard>
			{/each}
		</div>
	</CardContent>
</Card>
