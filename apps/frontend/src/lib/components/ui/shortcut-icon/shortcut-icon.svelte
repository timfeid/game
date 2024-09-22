<script lang="ts">
	import { onMount } from 'svelte';

	let isMac = false;
	onMount(() => (isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0));
</script>

<div class="absolute right-3 top-1/2 transform -translate-y-1/2 flex items-center space-x-1">
	<kbd
		class="py-0.5 px-1.5 text-center text-xs font-semibold dark:text-stone-500 text-stone-800 dark:bg-stone-900/50 bg-stone-100 border dark:border-stone-900/80 rounded-lg font-sans"
	>
		{isMac ? '⌘' : 'Ctrl'} + K
	</kbd>
</div>
