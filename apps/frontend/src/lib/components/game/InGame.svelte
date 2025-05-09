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
	let battlefieldStyle = '';
	let battlefield: HTMLDivElement;
	// function updatePerspective(e: MouseEvent) {
	// 	const contentHeight = battlefield?.scrollHeight || 0; // Get content height
	// 	const perspective = contentHeight * 2; // Adjust multiplier as needed

	// 	const screenHeight = window.innerHeight;

	// 	const y = e.clientY / screenHeight; // Normalize cursor Y position to [0, 1]

	// 	// Translate battlefield along Y and apply consistent rotation
	// 	const translateY = y * contentHeight * 0.25; // Adjust the depth effect
	// 	const rotateX = 40; // Fixed tilt

	// 	battlefieldStyle = `
	// 	transform: perspective(${perspective}px) translateY(${-translateY}px) rotateX(${rotateX}deg);
	// 	transform-origin: center center;
	// 	max-height: 100vh;
	// 	overflow: visible;
	// `;
	// }
</script>

<!-- <svelte:document onmousemove={updatePerspective} /> -->

<AttackerLines game={game_state} />

<div class="min-h-[calc(100vh)] flex w-full overflow-hidden">
	<div class="flex-grow">
		<div class=" z-50 absolute top-0 flex left-0 right-[300px]">
			<div class="container !px-3">
				<!-- <div class="h-20 flex items-center w-full min-w-full"> -->
				<PriorityQueueNotification {turnMessage} game={game_state}></PriorityQueueNotification>
				<!-- </div> -->
				{#if game_state.public_info.current_turn}
					<div class="text-center py-4 text-2xl uppercase dark:gray-950 font-serif dark:text-white">
						Turn #{game_state.public_info.current_turn.turn_number},
						{currentPlayer(game_state.public_info)}'s
						{game_state.public_info.current_turn.phase}
					</div>
				{/if}
			</div>
		</div>
		<div class="w-full flex min-h-full h-full max-h-full flex-grow !px-3 pt-6 overflow-hidden">
			<div class="flex flex-col flex-grow" bind:this={battlefield} style="  {battlefieldStyle}">
				{#each Object.keys(game_state.players) as key, index}
					{@const player = game_state.players[key]}
					<Player {index} code={join_code} game={game_state} {player} playerName={key} />
				{/each}
			</div>
		</div>
	</div>
	<div class=" !px-3 left-[300pmx-autox] pt-2 flex items-center">
		{#if isMyTurn}
			<Button
				class="ml-auto h-12 text-2xl  tracking-tight  font-medium rounded-full"
				on:click={turn}
			>
				<ArrowBigRight size={28} />
			</Button>
		{/if}
	</div>

	<div class="w-[262px]">
		<div class="!px-3 mx-auto py-2 w-full bg-gray-100 dark:bg-gray-950 h-screen flex">
			<div class="flex flex-wrap gap-1 items-center justify-center my-auto w-full">
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
