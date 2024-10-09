<script lang="ts">
	import type { AbilityDetails } from '@gangsta/rusty';
	import { CornerDownLeft } from 'lucide-svelte';
	import ManaBubble from '../mana-bubble.svelte';

	export let ability: AbilityDetails;
</script>

<div class="flex items-start">
	{#if ability.action_type == 'Tap'}
		<CornerDownLeft size="10" class="rotate-180 mt-1 shrink-0 mr-1" />
	{/if}

	{#each ability.mana_cost as mana}
		<ManaBubble color={mana} />
	{/each}

	<div class:text-muted={!ability.meets_requirements}>
		{ability.description}
	</div>
</div>
