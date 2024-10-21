<script lang="ts">
	import type { DeckSelector, ManaType } from '@gangsta/rusty';

	export let color: ManaType;
	export let count: number | undefined = undefined;

	export let className: string = '';
	export { className as class };
</script>

<div
	class="w-3 h-3 flex items-center justify-center rounded-full border border-black dark:border-white shrink-0 font-mono text-[10px] relative font-medium {className}"
	class:bg-green-400={color.toLowerCase() === 'green'}
	class:bg-red-400={color.toLowerCase() === 'red'}
	class:bg-blue-400={color.toLowerCase() === 'blue'}
	class:bg-gray-950={color.toLowerCase() === 'black'}
	class:bg-white={color.toLowerCase() === 'white'}
	class:dark:border-0={color.toLowerCase() === 'white'}
	class:!border-gray-500={color.toLowerCase() === 'colorless'}
>
	<div>
		{#if (count || 0) > 1}
			{count}
		{/if}
	</div>
</div>
