<script lang="ts">
	import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import type { AbilityDetails, GameState } from '@gangsta/rusty';
	import { onMount } from 'svelte';
	import { selectedAbility, selectFromAbilities } from '../../../stores/dialog';
	import Ability from '../card/ability.svelte';

	let abilities: AbilityDetails[] | undefined;
	export let game: GameState;
	export let code: string;

	function cancel() {
		open = false;
	}

	onMount(() => {
		return selectFromAbilities.subscribe((incoming) => {
			if (incoming) {
				open = true;
			}
			abilities = incoming;
		});
	});

	function selectAbility(ability: AbilityDetails) {
		selectedAbility.set(ability);
		open = false;
	}

	let open = true;
</script>

{#if abilities}
	<AlertDialog.Root bind:open>
		<AlertDialog.Trigger asChild let:builder>
			<Button builders={[builder]} variant="outline">Show Dialog</Button>
		</AlertDialog.Trigger>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>Choose an ability</AlertDialog.Title>
				<ul class="border rounded">
					{#each abilities as ability}
						<li class="border-b px-2 last:border-b-0 py-2">
							<button on:click={() => selectAbility(ability)}>
								<Ability noTooltips {ability}></Ability>
							</button>
						</li>
					{/each}
				</ul>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Action on:click={cancel}>Cancel</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
{/if}
