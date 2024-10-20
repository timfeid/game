<script lang="ts">
	import type { AbilityDetails } from '@gangsta/rusty';
	import { CornerDownLeft } from 'lucide-svelte';
	import ManaBubble from '../mana-bubble.svelte';
	import { Tooltip, TooltipTrigger } from '../../ui/tooltip';
	import TooltipContent from '../../ui/tooltip/tooltip-content.svelte';
	import Fragment from '../../fragment.svelte';

	export let ability: AbilityDetails;
	export let inHand: boolean = false;
	export let noTooltips = false;
</script>

<Tooltip>
	<svelte:component this={noTooltips ? Fragment : TooltipTrigger} class=" text-left line-clamp-3">
		{#if ability.action_type == 'Tap'}
			<CornerDownLeft size="10" class="rotate-180 shrink-0 inline" />
		{/if}

		{#each ability.mana_cost as mana}
			<ManaBubble class="inline-block" color={mana} />&nbsp;
		{/each}

		<span class:text-muted={!ability.meets_requirements_except_mana && !inHand} class="text-left">
			{ability.description}
		</span>
	</svelte:component>
	<TooltipContent class="max-w-[10rem] text-center">
		{#if ability.meets_requirements_except_mana && ability.meets_mana_requirements}
			This ability is ready!
		{:else if !ability.meets_requirements_except_mana}
			This ability is not ready
		{:else if ability.can_pay_mana}
			This ability is available. Please pay mana cost first.
		{:else if !ability.meets_mana_requirements}
			This ability is available, but you have no mana
		{:else}
			This ability is not ready
		{/if}
	</TooltipContent>
</Tooltip>
