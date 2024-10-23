<script lang="ts">
	import type { GameState, LobbyTurnMessage } from '@gangsta/rusty';

	export let game: GameState;
	export let turnMessage: LobbyTurnMessage | undefined;

	$: console.log(game.public_info.priority_queue);
</script>

{#if game.public_info.priority_queue}
	<div class="text-center">
		{game.public_info.priority_queue?.player_id || ''}'s priority queue
		{game.public_info.priority_queue.time_left}s
	</div>
{/if}
{#if turnMessage}
	<div
		class="relative text-center text-xs w-full flex flex-col justify-end h-16 pb-6 overflow-hidden"
	>
		{#each turnMessage.messages as message, index}
			<div
				class="{index >= turnMessage.messages.length - 2
					? index >= turnMessage.messages.length - 1
						? 'transform scale-125'
						: 'transform scale-110'
					: ''}  px-12 mx-auto"
			>
				{message}
			</div>
		{/each}
		<!-- Overlay for the fade effect -->
		<div
			class=" test absolute top-0 left-0 w-full h-16 bg-gradient-to-b from-white dark:from-gray-950 to-transparent pointer-events-none"
		></div>
	</div>
{/if}

<style lang="css">
	.test {
		-webkit-background-clip: text;
		background-clip: text;
		-webkit-text-fill-color: transparent;
	}
</style>
