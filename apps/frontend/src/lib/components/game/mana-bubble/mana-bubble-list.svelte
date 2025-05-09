<script lang="ts">
	import type { DeckSelector, ManaType } from '@gangsta/rusty';
	import ManaBubble from './mana-bubble.svelte';

	export let mana: ManaType[];

	export let className: string = '';
	export { className as class };
	export let big = false;

	$: colorlessCount = mana.filter((m) => m === 'Colorless').length;
	$: nonColorlessMana = mana.filter((m) => m !== 'Colorless');
</script>

<div class="inline-flex space-x-[2px] {className}">
	{#if colorlessCount > 0}
		<ManaBubble
			class={big ? ' top-[-2px] !h-[16px] !w-[16px]' : ''}
			color="Colorless"
			count={colorlessCount}
		/>
	{/if}
	{#each nonColorlessMana as color}
		<ManaBubble class={big ? ' top-[-2px] !h-[16px] !w-[16px]' : ''} {color} />
	{/each}
</div>
