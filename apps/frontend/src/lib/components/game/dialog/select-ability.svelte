<script lang="ts">
	import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import type { AbilityDetails, GameState } from '@gangsta/rusty';
	import { onMount } from 'svelte';
	import { selectedAbility, selectFromAbilities } from '../../../stores/dialog';
	import Ability from '../card/ability.svelte';
	import AlertDialogDescription from '../../ui/alert-dialog/alert-dialog-description.svelte';

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
	$: if (!open) {
		selectFromAbilities.set(undefined);
	}

	let open = true;
</script>

{#if abilities}
	<AlertDialog.Root bind:open>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>Choose an ability</AlertDialog.Title>
				<ul class="">
					{#each abilities as ability}
						<li class="border-b px-1 last:border-b-0 py-1">
							<Button
								type="button"
								variant="ghost"
								class="w-full justify-start text-left whitespace-normal h-auto"
								on:click={() => selectAbility(ability)}
							>
								<Ability noTooltips {ability}></Ability>
							</Button>
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
