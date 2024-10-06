<script lang="ts">
	import type { DeckSelector, ManaType } from '@gangsta/rusty';

	export let color: ManaType;
</script>

<div
	class="w-3 h-3 rounded-full border border-black"
	class:bg-green-400={color.toLowerCase() === 'green'}
	class:bg-blue-400={color.toLowerCase() === 'blue'}
	class:bg-gray-950={color.toLowerCase() === 'black'}
	class:bg-white={color.toLowerCase() === 'white'}
	class:dark:border-0={color.toLowerCase() === 'white'}
	class:bg-gray-500={color.toLowerCase() === 'colorless'}
></div>
