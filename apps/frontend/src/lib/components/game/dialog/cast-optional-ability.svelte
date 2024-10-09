<script lang="ts">
	import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import ManaBubble from '../mana-bubble.svelte';
	import { askOptionalAbility } from '../../../stores/dialog';
	import { onMount } from 'svelte';
	import type { ExecuteAbility, GameState } from '@gangsta/rusty';
	import { client } from '../../../client';
	import { waitForTarget } from '../game';

	let ability: ExecuteAbility | undefined = undefined;
	export let game: GameState;
	export let code: string;

	async function yes() {
		open = false;
		if (!ability) {
			return;
		}
		// console.log(ability.details)

		const target = await waitForTarget(ability.details, game);
		return await client.mutation([
			'lobby.respond_optional_ability',
			{ code, target, ability_id: ability.details.id, response: true }
		]);
	}

	async function no() {
		open = false;
		if (!ability) {
			return;
		}

		return await client.mutation([
			'lobby.respond_optional_ability',
			{ code, target: null, ability_id: ability.details.id, response: true }
		]);
	}

	onMount(() => {
		return askOptionalAbility.subscribe((incoming) => {
			if (incoming) {
				open = true;
			}
			ability = incoming;
		});
	});
	let open = false;
</script>

{#if ability}
	<AlertDialog.Root bind:open>
		<AlertDialog.Trigger asChild let:builder>
			<Button builders={[builder]} variant="outline">Show Dialog</Button>
		</AlertDialog.Trigger>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>
					{ability.card.card.name}
				</AlertDialog.Title>
				<AlertDialog.Description>
					{ability.details.description}
					Would you like to execute this ability for
					{#each ability.details.mana_cost as mana}
						<ManaBubble color={mana} />
					{/each}
					?
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel on:click={no}>Cancel</AlertDialog.Cancel>
				<AlertDialog.Action on:click={yes}>Continue</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
{/if}
