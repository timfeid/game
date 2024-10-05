<script lang="ts">
	import type { GameState, LobbyTurnMessage } from '@gangsta/rusty';

	export let game: GameState;
	export let turnMessage: LobbyTurnMessage | undefined;

	$: player =
		game.public_info.priority_queue?.player_index !== undefined
			? Object.keys(game.players).find(
					(p) => game.players[p].player_index === game.public_info.priority_queue!.player_index
				)
			: null;
</script>

<div class="col-span-4">
	{#if game.public_info.priority_queue}
		<div class="text-center">
			{player}'s priority queue
			{game.public_info.priority_queue.time_left}s
		</div>
	{:else if turnMessage}
		<div
			class="relative text-center text-xs w-full flex flex-col justify-end h-16 pb-6 overflow-hidden"
		>
			{#each turnMessage.messages as message, index}
				<div
					class="{index >= turnMessage.messages.length - 2
						? index >= turnMessage.messages.length - 1
							? 'transform scale-125'
							: 'transform scale-110'
						: ''} max-w-lg px-12 mx-auto"
				>
					{message}
				</div>
			{/each}
			<!-- Overlay for the fade effect -->
			<div
				class="absolute top-0 left-0 w-full h-16 bg-gradient-to-b from-white dark:from-gray-950 to-transparent pointer-events-none"
			></div>
		</div>
	{/if}
</div>
