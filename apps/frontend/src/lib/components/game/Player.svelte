<script lang="ts">
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import HeartPulse from 'lucide-svelte/icons/heart-pulse';
	import type {
		ActionCardTarget,
		CardRequiredTarget,
		CardWithDetails,
		GameState,
		PlayerState
	} from '@gangsta/rusty';
	import CCard from './Card.svelte';
	import { RectangleVertical } from 'lucide-svelte';
	import { user } from '../../stores/access-token';
	import { client } from '../../client';
	import { RSPCError } from '@rspc/client';
	import { toast } from 'svelte-sonner';
	import { writable, type Writable } from 'svelte/store';
	import type { EffectTarget } from './type';
	import CircularProgress from '../ui/circular-progress/circular-progress.svelte';
	import { convertEffectTarget, searchingForTarget, target, waitForTarget } from './game';

	export let game: GameState;
	export let code: string;
	export let playerName: string;
	export let player: PlayerState;

	$: self = game.players[$user?.sub || ''];

	async function executeAction(index: number, card: CardWithDetails, target?: EffectTarget) {
		try {
			await client.mutation([
				card.action_type === 'Attach' ? 'lobby.attach_card' : 'lobby.action_card',
				{
					code,
					player_index: player.player_index,
					in_play_index: index,
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

	async function actionCard(index: number) {
		console.log(index, 'was clicked on', playerName, player);
		console.log(self);
		if ($searchingForTarget) {
			console.log('set target.');
			target.set({ name: 'card', player_index: player.player_index, card_index: index });
			return;
		}
		const card = player.public_info.cards_in_play[index];
		if (self.player_index === player.player_index) {
			const target = await waitForTarget(card, game);
			return await executeAction(index, card, target);
		}
	}

	async function setPlayerTarget(player: PlayerState) {
		target.set({ name: 'player', index: player.player_index });
	}
</script>

<Card class="bg-gray-950">
	<CardHeader class="space-y-1">
		<CardTitle class="text-2xl font-bold text-center flex items-center">
			<button on:click={() => setPlayerTarget(player)}>
				<div class="mr-4">
					{playerName}
				</div>
			</button>
			{#each { length: player.public_info.hand_size } as _}
				<RectangleVertical />
			{/each}
			<div class="ml-auto">
				<div class="flex items-center">
					<HeartPulse />
					<div class="ml-2">
						{player.public_info.health}
					</div>
				</div>
			</div>
		</CardTitle>
	</CardHeader>
	<CardContent class="space-y-4">
		<div class="grid grid-cols-7 gap-2">
			{#each player.public_info.cards_in_play as card, i}
				<CCard on:click={() => actionCard(i)} cardWithDetails={card}></CCard>
			{/each}
		</div>
	</CardContent>
</Card>
