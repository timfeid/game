<script lang="ts">
	import Search from 'lucide-svelte/icons/search';

	import { Input, type InputEvents } from '../input';
	import type { HTMLInputAttributes } from 'svelte/elements';
	import ShortcutIcon from '../shortcut-icon/shortcut-icon.svelte';

	type $$Props = HTMLInputAttributes;
	type $$Events = InputEvents;

	let className: $$Props['class'] = undefined;
	export let value: $$Props['value'] = '';
	export { className as class };
</script>

<div class="{className} relative w-full">
	<Input
		{...$$restProps}
		bind:value
		type="text"
		placeholder="Search..."
		class="pl-10 pr-4 py-2 w-full"
	/>
	<Search class="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
	<ShortcutIcon></ShortcutIcon>
</div>
