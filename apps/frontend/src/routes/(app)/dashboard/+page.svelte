<script lang="ts">
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { client } from '../../../lib/client';

	async function createGame() {
		const response = await client.mutation(['lobby.create', []]);
		goto(`/game/${response.join_code}`);
	}
</script>

<Button on:click={createGame}>Create a game</Button>
