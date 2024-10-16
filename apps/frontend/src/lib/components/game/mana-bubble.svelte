<script lang="ts">
	import type { DeckSelector, ManaType } from '@gangsta/rusty';

	export let color: ManaType;

	export let className: string = '';
	export { className as class };
</script>

<div
	class="w-2.5 h-2.5 rounded-full border border-black dark:border-white shrink-0 {className}"
	class:bg-green-400={color.toLowerCase() === 'green'}
	class:bg-red-400={color.toLowerCase() === 'red'}
	class:bg-blue-400={color.toLowerCase() === 'blue'}
	class:bg-gray-950={color.toLowerCase() === 'black'}
	class:bg-white={color.toLowerCase() === 'white'}
	class:dark:border-0={color.toLowerCase() === 'white'}
	class:bg-gray-500={color.toLowerCase() === 'colorless'}
></div>
