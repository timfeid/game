<script lang="ts">
	import type { DeckSelector, ManaType } from '@gangsta/rusty';
	import { manaColors } from '../../../colors';

	interface Props {
		color: ManaType;
		count?: number;
		class?: string;
	}

	let { color, count, class: className }: Props = $props();

	const hexColor = $derived(manaColors[color.toLowerCase()]);
</script>

<div
	class="w-3 h-3 flex items-center justify-center rounded-full border border-black dark:border-white shrink-0 font-mono text-[10px] relative font-medium {className}"
	class:dark:border-0={color.toLowerCase() === 'white'}
	style="background: {hexColor}; "
>
	<div>
		{#if (count || 0) > 1}
			{count}
		{/if}
	</div>
</div>
