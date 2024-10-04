<script lang="ts">
	import NavLink from '$lib/components/ui/nav-link.svelte';
	import Menu from '$lib/components/ui/page/menu.svelte';
</script>

<div class="sticky top-0 w-full border-b flex h-12 items-center text-sm bg-secondary">
	<div class="container items-center flex w-full !px-3">
		<div class="text-muted">gangsta</div>
		<div class="ml-auto">
			<Menu />
		</div>
	</div>
</div>

<slot />
