<script lang="ts">
	import NavLink from '$lib/components/ui/nav-link.svelte';
	import Menu from '$lib/components/ui/page/menu.svelte';
</script>

<div class="fixed z-[100] top-2 right-2">
	<Menu />
</div>

<slot />
