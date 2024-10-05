<script lang="ts">
	import type { DeckSelector, ManaType } from '@gangsta/rusty';

	export let color: ManaType;
</script>

<div
	class="w-3 h-3 rounded-full"
	class:bg-green-400={color.toLowerCase() === 'green'}
	class:bg-blue-400={color.toLowerCase() === 'blue'}
	class:bg-gray-950={color.toLowerCase() === 'black'}
></div>
