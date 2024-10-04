<script lang="ts">
	import type { GameState } from '@gangsta/rusty';

	export let game: GameState;

	$: player =
		game.public_info.priority_queue?.player_index !== undefined
			? Object.keys(game.players).find(
					(p) => game.players[p].player_index === game.public_info.priority_queue!.player_index
				)
			: null;
</script>

<div class="mx-auto">
	{#if game.public_info.priority_queue}
		{player}'s priority queue
		{game.public_info.priority_queue.time_left}s
	{/if}
</div>
