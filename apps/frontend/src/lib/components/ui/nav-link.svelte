<script lang="ts">
	import { page } from '$app/stores';

	type $$Props = { href: string };
	export let href: $$Props['href'];
	$: active = $page?.url?.pathname == href;
</script>

<a {href} class:dark:text-white={active} class="dark:text-gray-500"><slot /></a>
