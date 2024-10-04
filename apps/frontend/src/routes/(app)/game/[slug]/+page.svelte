<script lang="ts">
	import { page } from '$app/stores';
	import Game from '$lib/components/game/Game.svelte';
</script>

<Game code={$page.params.slug} />
