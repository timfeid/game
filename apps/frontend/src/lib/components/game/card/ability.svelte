<script lang="ts">
	import type { AbilityDetails } from '@gangsta/rusty';
	import { CornerDownLeft } from 'lucide-svelte';
	import ManaBubble from '../mana-bubble/mana-bubble.svelte';
	import { Tooltip, TooltipTrigger } from '../../ui/tooltip';
	import TooltipContent from '../../ui/tooltip/tooltip-content.svelte';
	import Fragment from '../../fragment.svelte';
	import ManaBubbleList from '../mana-bubble/mana-bubble-list.svelte';

	export let ability: AbilityDetails;
	export let inHand: boolean = false;
	export let noTooltips = false;
	export let offsetTop = false;
</script>

<Tooltip>
	<svelte:component
		this={noTooltips ? Fragment : TooltipTrigger}
		class="min-h-[16px] text-left line-clamp-3 relative space-x-1"
	>
		{#if ability.action_type == 'Tap'}
			<CornerDownLeft size="10" class="rotate-180 shrink-0 inline-block float-left" />
		{/if}

		<ManaBubbleList class="relative top-[2px] float-left" mana={ability.mana_cost} />

		<span
			class:text-muted={(!ability.meets_requirements_except_mana ||
				!ability.meets_mana_requirements) &&
				!inHand &&
				!noTooltips}
			class="leading-[16px] min-h-[16px] text-left">{ability.description}</span
		>
	</svelte:component>
	<TooltipContent class="max-w-[13rem] text-left text-base" side="bottom">
		<div class="border-b pb-2 mb-2 border-gray-300">
			{#if ability.action_type == 'Tap'}
				<CornerDownLeft size="10" class="rotate-180 shrink-0 inline" />
			{/if}

			<ManaBubbleList
				class={offsetTop && ability.mana_cost.length ? '' : ''}
				big
				mana={ability.mana_cost}
			/>

			<span class="leading-[16px] text-left">
				{ability.description}
			</span>
		</div>
		<div class="">
			{#if ability.meets_requirements_except_mana && ability.meets_mana_requirements}
				This ability is ready!
			{:else if !ability.meets_requirements_except_mana}
				This ability is not ready.
			{:else if ability.can_pay_mana}
				This ability is available. Please pay mana cost first.
			{:else if !ability.meets_mana_requirements}
				This ability is available, but you have no mana.
			{:else}
				This ability is not ready.
			{/if}
		</div>
	</TooltipContent>
</Tooltip>
