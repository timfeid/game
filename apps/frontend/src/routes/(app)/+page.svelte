<script lang="ts">
	import { onMount } from 'svelte';
	import { accessToken } from '../../lib/stores/access-token';
	import { goto } from '$app/navigation';

	onMount(() => {
		if (!$accessToken) {
			return goto('/login');
		}
		return goto('/dashboard');
	});
</script>
