<script lang="ts">
	import type { GameState, LobbyTurnMessage, PublicGameInfo } from '@gangsta/rusty';
	import { ArrowBigRight } from 'lucide-svelte';
	import { client } from '../../client';
	import { user } from '../../stores/access-token';
	import Button from '../ui/button/button.svelte';
	import CCard from './card/playing-card.svelte';
	import CastMandatoryAbility from './dialog/cast-mandatory-ability.svelte';
	import AskOptionalAbility from './dialog/cast-optional-ability.svelte';
	import SelectAbility from './dialog/select-ability.svelte';
	import SelectCard from './dialog/select-card.svelte';
	import Player from './Player.svelte';
	import PriorityQueueNotification from './priority-queue-notification.svelte';
	import AttackerLines from './attacker-lines.svelte';
	import MiniGame from './dialog/mini-game.svelte';

	export let game_state: GameState;
	export let turnMessage: LobbyTurnMessage | undefined;
	export let join_code: string;

	$: self = game_state.players[$user?.sub || ''];

	$: isMyTurn = self.sub === game_state.public_info.current_turn?.current_player_id;

	async function turn() {
		await client.mutation(['lobby.turn', join_code]);
	}

	function currentPlayer(info: PublicGameInfo) {
		for (const k of Object.keys(game_state.players)) {
			if (game_state.players[k].sub === info.current_turn?.current_player_id) {
				return k;
			}
		}
		return '';
	}
</script>

<AttackerLines game={game_state} />

<div class="min-h-[calc(100vh)] flex flex-col w-full">
	<div
		class=" z-50 sticky top-0 bg-gradient-to-r from-red-500 from-10% to via-purple-700 via-50% to-indigo-500 to-90% flex shadow-xl"
	>
		<div class="container !px-3 pb-6">
			<div class="h-24 flex items-center w-full min-w-full">
				<PriorityQueueNotification {turnMessage} game={game_state}></PriorityQueueNotification>
			</div>
			{#if game_state.public_info.current_turn}
				<div class="text-center text-4xl uppercase dark:gray-950 font-serif text-white">
					Turn #{game_state.public_info.current_turn.turn_number},
					{currentPlayer(game_state.public_info)}'s
					{game_state.public_info.current_turn.phase}
				</div>
			{/if}
		</div>
	</div>
	<div class="w-full flex-grow !px-3 pt-6">
		<div class="grid grid-cols-2 gap-3">
			{#each Object.keys(game_state.players) as key}
				{@const player = game_state.players[key]}
				<Player code={join_code} game={game_state} {player} playerName={key} />
			{/each}
		</div>
	</div>

	<div class="z-40 sticky bottom-0 left-0 right-0">
		<div class="container !px-3 mx-auto pt-2 flex items-center">
			{#if isMyTurn}
				<Button
					class="ml-auto h-12 text-2xl  tracking-tight  font-medium rounded-full"
					on:click={turn}
				>
					<ArrowBigRight size={28} />
				</Button>
			{/if}
		</div>
		<div class="!px-3 mx-auto py-2 w-full bg-gray-100 dark:bg-gray-950 border-t mt-4">
			<div class="flex flex-wrap gap-1 justify-center w-full">
				{#each self.hand as card, i}
					<CCard game={game_state} cardWithDetails={card}></CCard>
				{/each}
			</div>
		</div>
	</div>
</div>
<!-- <pre>{JSON.stringify(self)}</pre> -->
<AskOptionalAbility code={join_code} game={game_state} />
<CastMandatoryAbility code={join_code} game={game_state} />
<SelectAbility code={join_code} game={game_state} />
<SelectCard code={join_code} game={game_state} />
<MiniGame code={join_code} game={game_state} />
