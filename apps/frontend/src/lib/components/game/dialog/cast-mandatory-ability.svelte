<script lang="ts">
	import type { ExecuteAbility, GameState } from '@gangsta/rusty';
	import { onMount } from 'svelte';
	import { client } from '../../../client';
	import { mandatoryAbility } from '../../../stores/dialog';
	import { waitForTarget } from '../game';
	import * as AlertDialog from '../../ui/alert-dialog';
	import ManaBubble from '../mana-bubble/mana-bubble.svelte';

	let ability: ExecuteAbility | undefined = undefined;
	export let game: GameState;
	export let code: string;

	async function yes() {
		if (!ability) {
			return;
		}

		const target = await waitForTarget(ability.details, game);
		return await client.mutation([
			'lobby.respond.mandatory_ability',
			{ code, target, ability_id: ability.details.id }
		]);
	}

	onMount(() => {
		return mandatoryAbility.subscribe((incoming) => {
			if (incoming) {
				ability = incoming;
				open = true;
			}
		});
	});
	let open = false;

	$: if (!open) {
		mandatoryAbility.set(undefined);
	}
</script>

{#if ability}
	<AlertDialog.Root bind:open>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>
					{ability.card.card.name}
				</AlertDialog.Title>
				<AlertDialog.Description>
					{ability.details.description}
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Action on:click={yes}>Continue</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
{/if}
