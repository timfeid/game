<script lang="ts">
	import type { FrontendTarget, GameState, LobbyTurnMessage, PublicGameInfo } from '@gangsta/rusty';
	import { RSPCError } from '@rspc/client';
	import { toast } from 'svelte-sonner';
	import { client } from '../../client';
	import { user } from '../../stores/access-token';
	import Button from '../ui/button/button.svelte';
	import CCard from './Card.svelte';
	import { waitForTarget } from './game';
	import Player from './Player.svelte';
	import PriorityQueueNotification from './priority-queue-notification.svelte';
	import { ArrowBigRight } from 'lucide-svelte';
	import AskOptionalAbility from './dialog/cast-optional-ability.svelte';
	import CastMandatoryAbility from './dialog/cast-mandatory-ability.svelte';

	export let game_state: GameState;
	export let turnMessage: LobbyTurnMessage | undefined;
	export let join_code: string;

	$: self = game_state.players[$user?.sub || ''];

	$: isMyTurn = self.player_index === game_state.public_info.current_turn?.current_player_index;

	async function turn() {
		await client.mutation(['lobby.turn', join_code]);
	}

	async function executePlayCard(index: number, target: FrontendTarget | null) {
		try {
			await client.mutation([
				'lobby.play_card',
				{
					code: join_code,
					in_hand_index: index,
					target: target
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
		const target = await waitForTarget(card.abilities[0], game_state, true);
		return await executePlayCard(index, target);
	}

	function currentPlayer(info: PublicGameInfo) {
		for (const k of Object.keys(game_state.players)) {
			if (game_state.players[k].player_index === info.current_turn?.current_player_index) {
				return k;
			}
		}
		return '';
	}
</script>

<div class="min-h-[calc(100vh-3rem)] flex flex-col w-full">
	<div class="flex-grow flex">
		<div class="container !px-3">
			<div class="h-24 flex items-center grid grid-cols-8 w-full">
				{#if isMyTurn}
					<Button on:click={turn}>
						Advance turn
						<ArrowBigRight class="pl-1" />
					</Button>
				{/if}
				<PriorityQueueNotification {turnMessage} game={game_state}></PriorityQueueNotification>
				{#if game_state.public_info.current_turn}
					<div class="col-span-3 text-right">
						Turn #{game_state.public_info.current_turn.turn_number},
						{currentPlayer(game_state.public_info)}'s
						{game_state.public_info.current_turn.phase}
					</div>
				{/if}
			</div>

			<div class="space-y-3">
				{#each Object.keys(game_state.players) as key}
					{@const player = game_state.players[key]}
					<Player code={join_code} game={game_state} {player} playerName={key} />
				{/each}
			</div>
		</div>
	</div>

	<div class="z-50 sticky bottom-0 left-0 right-0 bg-gray-100 dark:bg-gray-950 border-t mt-4">
		<div class="container !px-3 relative flex mx-auto py-2">
			<div class="grid gap-2 grid-cols-7">
				{#each self.hand as card, i}
					<CCard
						pile="Hand"
						playerIndex={self.player_index}
						cardIndex={i}
						on:click={() => playCard(i)}
						game={game_state}
						cardWithDetails={card}
					></CCard>
				{/each}
			</div>
		</div>
	</div>
</div>
<!-- <pre>{JSON.stringify(self)}</pre> -->
<AskOptionalAbility code={join_code} game={game_state} />
<CastMandatoryAbility code={join_code} game={game_state} />
