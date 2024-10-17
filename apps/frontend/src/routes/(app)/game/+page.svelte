<script lang="ts">
	import { page } from '$app/stores';
	import Game from '$lib/components/game/Game.svelte';

	$: code = $page.url.searchParams.get('code');
</script>

{#if code}
	<Game {code} />
{/if}
